//! 内容识别填充（类似 PS 的 Content-Aware Fill）
//!
//! 算法：多尺度 PatchMatch + EM 重建（Wexler/Barnes 一路的思路），纯 Rust、零依赖。
//!
//! 1. 建图像金字塔（每次减半，最多 4 层）；
//! 2. 从最粗层开始：破洞处先用邻近已知像素填个底色（最粗层），
//!    之后每层都用上一层放大后的结果作为初始估计；
//! 3. 每层做若干次 EM：
//!    - E 步：给每个洞像素找一个最相似的已知 patch（PatchMatch 传播 + 随机搜索）；
//!    - M 步：把命中的 patch 按偏移投票平均回去，得到新的洞内像素；
//! 4. 逐层放大到原尺寸。
//!
//! 对「纯色/渐变背景上的文字水印」几乎无痕；照片里的中小物体会有轻微涂抹感，
//! 大片复杂纹理（人脸、密集树叶）做不到 PS 那种一键完美，这是算法本身的边界。

const PATCH_R: i32 = 3; // 7×7 的 patch
const MAX_LEVELS: usize = 4;
const MIN_SIDE: usize = 24;

struct Level {
    w: usize,
    h: usize,
    /// RGB 浮点（0..255）
    px: Vec<[f32; 3]>,
    /// 1 = 洞
    hole: Vec<u8>,
}

/// 简易伪随机（避免引入 rand 依赖），xorshift32
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self {
        Rng(seed | 1)
    }
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next() as usize) % n
    }
}

pub struct Report {
    pub ms: u64,
    pub hole_px: usize,
    pub levels: usize,
    pub region_w: usize,
    pub region_h: usize,
    pub mode: &'static str,
}

/// 智能模式：先在矩形选区里把「内容」挑出来，只擦内容，背景一个像素都不动。
///
/// 这就是「淡黄底上的黑字」要的效果：黑字被抹掉，淡黄底原样保留。
/// 判断不出背景（照片里的复杂纹理）时自动回退成整框重建。
pub fn inpaint_auto(px: &mut [u8], w: usize, h: usize, rect: &[u8], iters: usize) -> Result<Report, String> {
    if w == 0 || h == 0 {
        return Err("图像尺寸为空".into());
    }
    if px.len() != w * h * 4 {
        return Err(format!("像素长度不符：{} != {}", px.len(), w * h * 4));
    }
    if rect.len() != w * h {
        return Err(format!("mask 长度不符：{} != {}", rect.len(), w * h));
    }
    let rect_px = rect.iter().filter(|v| **v != 0).count();
    if rect_px == 0 {
        return Err("没有选中要填充的区域".into());
    }

    // 只有「背景确实占多数且颜色集中」时才敢只擦内容
    if let Some((cm, bg_sd)) = content_mask(px, w, h, rect) {
        let content_px = cm.iter().filter(|v| **v != 0).count();
        if content_px > 0 {
            let t0 = std::time::Instant::now();
            if bg_sd < 20.0 {
                blend_to_background(px, w, h, rect, &cm, bg_sd);
                return Ok(Report {
                    ms: t0.elapsed().as_millis() as u64,
                    hole_px: content_px,
                    levels: 1,
                    region_w: w,
                    region_h: h,
                    mode: "content",
                });
            }
            let rep = fill_holes(px, w, h, &cm, iters)?;
            return Ok(Report {
                hole_px: content_px,
                mode: if rep.mode == "patch" { "content-patch" } else { "content" },
                ..rep
            });
        }
    }

    inpaint(px, w, h, rect, iters)
}

fn to_level(px: &[u8], w: usize, h: usize, mask: &[u8]) -> Level {
    Level {
        w,
        h,
        px: (0..w * h)
            .map(|i| [px[i * 4] as f32, px[i * 4 + 1] as f32, px[i * 4 + 2] as f32])
            .collect(),
        hole: mask.to_vec(),
    }
}

/// 从「整块矩形选区」里分离出「内容蒙版」。
///
/// 文档/截图/纯色海报的共同特征：选区里背景像素占绝大多数。
/// 所以各通道取中位数就是背景色，再对「到背景色的距离」跑一次 Otsu，
/// 距离大的那些像素就是字/图形。返回 (内容蒙版, 背景颜色的标准差)。
///
/// 只要有一项不可信就返回 None，让上层回退到整框重建。
fn content_mask(px: &[u8], w: usize, h: usize, rect: &[u8]) -> Option<(Vec<u8>, f32)> {
    let n = w * h;
    let total = rect.iter().filter(|v| **v != 0).count();
    if total < 48 {
        return None;
    }

    // 1) 各通道中位数 = 背景色
    let mut hist = [[0u32; 256]; 3];
    for i in 0..n {
        if rect[i] == 0 {
            continue;
        }
        for c in 0..3 {
            hist[c][px[i * 4 + c] as usize] += 1;
        }
    }
    let mut bg = [0f32; 3];
    for c in 0..3 {
        let half = (total / 2) as u32;
        let mut acc = 0u32;
        for v in 0..256 {
            acc += hist[c][v];
            if acc > half {
                bg[c] = v as f32;
                break;
            }
        }
    }

    // 2) 到背景色的距离，量化到 0..255 做直方图
    let mut dq = vec![0u8; n];
    let mut dh = [0u32; 256];
    for i in 0..n {
        if rect[i] == 0 {
            continue;
        }
        let dr = px[i * 4] as f32 - bg[0];
        let dg = px[i * 4 + 1] as f32 - bg[1];
        let db = px[i * 4 + 2] as f32 - bg[2];
        let d = (dr * dr + dg * dg + db * db).sqrt();
        let q = (d * (255.0 / 441.673)).min(255.0) as u8;
        dq[i] = q;
        dh[q as usize] += 1;
    }

    // 3) Otsu 分界
    let mut t = otsu(&dh, total);
    // 阈值太低会把 JPEG 噪点、细微纹理当内容
    if t < 7 {
        t = 7;
    }

    // 4) 内容 = 距离超过阈值；同时统计背景的波动
    let mut mask = vec![0u8; n];
    let mut content = 0usize;
    let mut fsum = 0f64;
    let mut bs = [0f64; 3];
    let mut bs2 = [0f64; 3];
    let mut bn = 0f64;
    for i in 0..n {
        if rect[i] == 0 {
            continue;
        }
        if dq[i] as usize > t {
            mask[i] = 1;
            content += 1;
            fsum += dq[i] as f64;
        } else {
            for c in 0..3 {
                let v = px[i * 4 + c] as f64;
                bs[c] += v;
                bs2[c] += v * v;
            }
            bn += 1.0;
        }
    }
    if content == 0 || bn < 8.0 {
        return None;
    }
    let content_ratio = content as f32 / total as f32;
    // 选区内找不到成规模的内容，或内容比背景还多（中位数根本不是背景色）
    // → 说明这不是「纯色底 + 字」的场景，老老实实整框重建
    if content_ratio < 0.002 || content_ratio > 0.6 {
        return None;
    }
    // 内容本身离背景不够远，就是把压缩噪声硬切了一刀，别乱擦
    let f_mean = fsum / content as f64; // 量化后的平均距离
    if f_mean < 10.0 {
        return None;
    }

    let mut bg_sd = 0f32;
    if bn > 1.0 {
        for c in 0..3 {
            let mean = bs[c] / bn;
            let var = (bs2[c] / bn - mean * mean).max(0.0);
            bg_sd = bg_sd.max(var.sqrt() as f32);
        }
    }

    // 5) 膨胀两圈：抗锯齿的灰边落在阈值以内，不撑开就会留一圈淡影
    dilate(&mut mask, w, h, rect, 2);
    Some((mask, bg_sd))
}

/// 大津法：把直方图分成两类，取类间方差最大的分割点
fn otsu(hist: &[u32; 256], total: usize) -> usize {
    let sum_all: f64 = (0..256).map(|i| i as f64 * hist[i] as f64).sum();
    let mut sum_b = 0.0f64;
    let mut w_b = 0usize;
    let mut best = -1.0f64;
    let mut best_t = 0usize;
    for t in 0..256 {
        w_b += hist[t] as usize;
        if w_b == 0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0 {
            break;
        }
        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b as f64;
        let m_f = (sum_all - sum_b) / w_f as f64;
        let var = w_b as f64 * w_f as f64 * (m_b - m_f) * (m_b - m_f);
        if var > best {
            best = var;
            best_t = t;
        }
    }
    best_t
}

/// 只擦内容、背景原样保留：找到每个待填充像素「最近的那块干净背景」，再把它的颜色搬过来。
///
/// 为什么不用分块均值：16px 的块在被字盖满时会退化，取到的是四周块的均值，
/// 跟紧贴字边的真实底色差那么两三级，放大后会看到隐约的字形台阶。
/// 从每个干净背景像素出发做多源 BFS，谁先到就算谁的 —— 纯色底上结果与背景逐像素一致，
/// 渐变底上自然贴合，也不存在块边界。
///
/// 混合方式是软的：离底色近的像素一点都不动（背景 100% 保留），
/// 判定为内容的像素彻底换成底色，中间那圈抗锯齿/JPEG 灰雾按距离平滑过渡。
fn blend_to_background(px: &mut [u8], w: usize, h: usize, rect: &[u8], content: &[u8], bg_sd: f32) {
    let n = w * h;

    // 1) 干净背景像素的参考色（5×5 邻域均值，抹掉噪点，只统计同样是背景的像素）
    let mut refc = vec![[0f32; 3]; n];
    let mut is_src = vec![false; n];
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if rect[i] == 0 || content[i] != 0 {
                continue;
            }
            let mut s = [0f32; 3];
            let mut c = 0f32;
            for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let ni = (ny as usize) * w + nx as usize;
                    if rect[ni] == 0 || content[ni] != 0 {
                        continue;
                    }
                    for k in 0..3 {
                        s[k] += px[ni * 4 + k] as f32;
                    }
                    c += 1.0;
                }
            }
            if c > 0.0 {
                for k in 0..3 {
                    refc[i][k] = s[k] / c;
                }
                is_src[i] = true;
                any = true;
            }
        }
    }
    if !any {
        return;
    }

    // 2) 多源 BFS：洞内每个像素认领最近的那块背景
    let mut src = vec![usize::MAX; n];
    let mut q: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    for i in 0..n {
        if is_src[i] {
            src[i] = i;
            q.push_back(i);
        }
    }
    while let Some(i) = q.pop_front() {
        let x = i % w;
        let y = i / w;
        let s = src[i];
        let mut push = |ni: usize, q: &mut std::collections::VecDeque<usize>| {
            if rect[ni] != 0 && src[ni] == usize::MAX {
                src[ni] = s;
                q.push_back(ni);
            }
        };
        if x > 0 {
            push(i - 1, &mut q);
        }
        if x + 1 < w {
            push(i + 1, &mut q);
        }
        if y > 0 {
            push(i - w, &mut q);
        }
        if y + 1 < h {
            push(i + w, &mut q);
        }
    }

    // 3) 软混合：内容 → 直接换成底色；灰边 → 按距离渐变；纯背景 → 不动
    let lo = 8.0 + bg_sd;
    let hi = (lo * 3.0).max(36.0);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if rect[i] == 0 {
                continue;
            }
            let s = src[i];
            if s == usize::MAX {
                continue;
            }
            let b = refc[s];
            let d = {
                let dr = px[i * 4] as f32 - b[0];
                let dg = px[i * 4 + 1] as f32 - b[1];
                let db = px[i * 4 + 2] as f32 - b[2];
                (dr * dr + dg * dg + db * db).sqrt()
            };
            let a = if content[i] != 0 {
                1.0
            } else {
                ((d - lo) / (hi - lo)).clamp(0.0, 1.0)
            };
            if a <= 0.0 {
                continue;
            }
            for k in 0..3 {
                let v = px[i * 4 + k] as f32 * (1.0 - a) + b[k] * a;
                px[i * 4 + k] = v.clamp(0.0, 255.0).round() as u8;
            }
            if px[i * 4 + 3] == 0 {
                px[i * 4 + 3] = 255;
            }
        }
    }
}

/// 3×3 膨胀，但不越出矩形选区
fn dilate(mask: &mut [u8], w: usize, h: usize, rect: &[u8], times: usize) {
    for _ in 0..times {
        let src = mask.to_vec();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if src[i] != 0 || rect[i] == 0 {
                    continue;
                }
                let mut hit = false;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        if src[(ny as usize) * w + (nx as usize)] != 0 {
                            hit = true;
                            break;
                        }
                    }
                    if hit {
                        break;
                    }
                }
                if hit {
                    mask[i] = 1;
                }
            }
        }
    }
}

/// 输入：RGBA 像素（会就地在洞内改写 RGB），宽高，mask（每像素 1 字节，非 0 = 洞）
pub fn inpaint(px: &mut [u8], w: usize, h: usize, mask: &[u8], iters: usize) -> Result<Report, String> {
    fill_holes(px, w, h, mask, iters)
}

fn fill_holes(px: &mut [u8], w: usize, h: usize, mask: &[u8], iters: usize) -> Result<Report, String> {
    if w == 0 || h == 0 {
        return Err("图像尺寸为空".into());
    }
    if px.len() != w * h * 4 {
        return Err(format!("像素长度不符：{} != {}", px.len(), w * h * 4));
    }
    if mask.len() != w * h {
        return Err(format!("mask 长度不符：{} != {}", mask.len(), w * h));
    }

    let hole_px = mask.iter().filter(|v| **v != 0).count();
    if hole_px == 0 {
        return Err("没有选中要填充的区域".into());
    }

    let t0 = std::time::Instant::now();

    // ---- 快路径：背景是纯色/渐变（文档、截图、纯色海报）----
    // 这类图用「沿行/列插值 + 轻微平滑」就够了，效果比 PatchMatch 更干净，而且快两个数量级
    let mut base = Level {
        w,
        h,
        px: (0..w * h)
            .map(|i| {
                [
                    px[i * 4] as f32,
                    px[i * 4 + 1] as f32,
                    px[i * 4 + 2] as f32,
                ]
            })
            .collect(),
        hole: mask.to_vec(),
    };

    let smooth = background_is_smooth(&base);
    if smooth {
        fill_smooth(&mut base);
        write_back(px, &base, mask);
        return Ok(Report {
            ms: t0.elapsed().as_millis() as u64,
            hole_px,
            levels: 1,
            region_w: w,
            region_h: h,
            mode: "smooth",
        });
    }

    // ---- 精细路径：照片/纹理，多尺度 PatchMatch ----
    let mut levels: Vec<Level> = Vec::new();
    levels.push(base);
    while levels.len() < MAX_LEVELS {
        let last = levels.last().unwrap();
        let (nw, nh) = (last.w / 2, last.h / 2);
        if nw < MIN_SIDE || nh < MIN_SIDE {
            break;
        }
        let next = downsample(last, nw, nh);
        levels.push(next);
    }

    // ---- 从最粗层开始解 ----
    let n_levels = levels.len();
    // 最粗层先用边界已知像素的平均色垫底，避免初始值全黑把匹配带偏
    fill_with_boundary_mean(&mut levels[n_levels - 1]);

    let mut prev: Option<Level> = None;
    for li in (0..n_levels).rev() {
        if li != n_levels - 1 {
            // 放大上一层的解，作为本层初值
            let up = upsample(prev.as_ref().unwrap(), levels[li].w, levels[li].h);
            levels[li].px = up.px;
        }
        let it = if iters == 0 { 4 } else { iters };
        solve(&mut levels[li], it, li as u32 * 7919 + 13);
        prev = Some(Level {
            w: levels[li].w,
            h: levels[li].h,
            px: levels[li].px.clone(),
            hole: levels[li].hole.clone(),
        });
    }

    // ---- 写回 ----
    write_back(px, &levels[0], mask);

    Ok(Report {
        ms: t0.elapsed().as_millis() as u64,
        hole_px,
        levels: n_levels,
        region_w: w,
        region_h: h,
        mode: "patch",
    })
}

fn write_back(px: &mut [u8], l: &Level, mask: &[u8]) {
    for i in 0..l.w * l.h {
        if mask[i] == 0 {
            continue;
        }
        let c = l.px[i];
        px[i * 4] = c[0].clamp(0.0, 255.0).round() as u8;
        px[i * 4 + 1] = c[1].clamp(0.0, 255.0).round() as u8;
        px[i * 4 + 2] = c[2].clamp(0.0, 255.0).round() as u8;
        if px[i * 4 + 3] == 0 {
            px[i * 4 + 3] = 255;
        }
    }
}

/// 洞边缘一圈像素的颜色波动有多大？足够平就是纯色/渐变背景
fn background_is_smooth(l: &Level) -> bool {
    let (w, h) = (l.w, l.h);
    let mut sum = [0.0f64; 3];
    let mut sum2 = [0.0f64; 3];
    let mut n = 0.0f64;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            if l.hole[i] != 0 {
                continue;
            }
            let is_edge = l.hole[i - 1] != 0
                || l.hole[i + 1] != 0
                || l.hole[i - w] != 0
                || l.hole[i + w] != 0;
            if !is_edge {
                continue;
            }
            for c in 0..3 {
                let v = l.px[i][c] as f64;
                sum[c] += v;
                sum2[c] += v * v;
            }
            n += 1.0;
        }
    }
    if n < 8.0 {
        return false;
    }
    // 三个通道里最大的标准差
    let mut max_sd: f64 = 0.0;
    for c in 0..3 {
        let mean = sum[c] / n;
        let var = (sum2[c] / n - mean * mean).max(0.0);
        max_sd = max_sd.max(var.sqrt());
    }
    max_sd < 16.0
}

/// 沿行/列做线性插值，再平滑两遍：适合纯色与渐变背景
fn fill_smooth(l: &mut Level) {
    let (w, h) = (l.w, l.h);

    // 1) 先按列：每个洞段用上下最近的已知像素插值
    for x in 0..w {
        let mut y = 0;
        while y < h {
            if l.hole[y * w + x] == 0 {
                y += 1;
                continue;
            }
            let y0 = y;
            while y < h && l.hole[y * w + x] != 0 {
                y += 1;
            }
            let y1 = y; // 段为 [y0, y1)
            let above = if y0 > 0 { Some(y0 - 1) } else { None };
            let below = if y1 < h { Some(y1) } else { None };
            for yy in y0..y1 {
                let t = if let (Some(a), Some(b)) = (above, below) {
                    (yy - a) as f32 / (b - a) as f32
                } else {
                    0.5
                };
                let c = match (above, below) {
                    (Some(a), Some(b)) => {
                        lerp3(l.px[a * w + x], l.px[b * w + x], t)
                    }
                    (Some(a), None) => l.px[a * w + x],
                    (None, Some(b)) => l.px[b * w + x],
                    (None, None) => [0.0, 0.0, 0.0],
                };
                l.px[yy * w + x] = c;
            }
        }
    }

    // 2) 再按行补一遍：整行都是洞（列插值无从取值）时靠左右邻居
    for y in 0..h {
        let mut x = 0;
        while x < w {
            if l.hole[y * w + x] == 0 {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < w && l.hole[y * w + x] != 0 {
                x += 1;
            }
            let x1 = x;
            let left = if x0 > 0 { Some(x0 - 1) } else { None };
            let right = if x1 < w { Some(x1) } else { None };
            for xx in x0..x1 {
                let t = if let (Some(a), Some(b)) = (left, right) {
                    (xx - a) as f32 / (b - a) as f32
                } else {
                    0.5
                };
                l.px[y * w + xx] = match (left, right) {
                    (Some(a), Some(b)) => lerp3(l.px[y * w + a], l.px[y * w + b], t),
                    (Some(a), None) => l.px[y * w + a],
                    (None, Some(b)) => l.px[y * w + b],
                    (None, None) => l.px[y * w + xx],
                };
            }
        }
    }

    // 3) 两遍轻度平滑：抹掉插值留下的直线痕迹（只改洞内像素）
    for _ in 0..2 {
        let src = l.px.clone();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if l.hole[i] == 0 {
                    continue;
                }
                let mut acc = [0.0f32; 3];
                let mut n = 0.0f32;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        let s = src[(ny as usize) * w + (nx as usize)];
                        acc[0] += s[0];
                        acc[1] += s[1];
                        acc[2] += s[2];
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    l.px[i] = [acc[0] / n, acc[1] / n, acc[2] / n];
                }
            }
        }
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn downsample(src: &Level, nw: usize, nh: usize) -> Level {
    let mut px = vec![[0.0f32; 3]; nw * nh];
    let mut hole = vec![0u8; nw * nh];
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0.0f32; 3];
            let mut n = 0.0f32;
            let mut hc = 0;
            for dy in 0..2 {
                for dx in 0..2 {
                    let sx = (x * 2 + dx).min(src.w - 1);
                    let sy = (y * 2 + dy).min(src.h - 1);
                    let si = sy * src.w + sx;
                    let p = src.px[si];
                    acc[0] += p[0];
                    acc[1] += p[1];
                    acc[2] += p[2];
                    n += 1.0;
                    if src.hole[si] != 0 {
                        hc += 1;
                    }
                }
            }
            px[y * nw + x] = [acc[0] / n, acc[1] / n, acc[2] / n];
            // 只要有一半以上是洞就算洞（略微收缩，避免洞无限扩散）
            hole[y * nw + x] = if hc >= 2 { 1 } else { 0 };
        }
    }
    Level { w: nw, h: nh, px, hole }
}

fn upsample(src: &Level, w: usize, h: usize) -> Level {
    let mut px = vec![[0.0f32; 3]; w * h];
    let mut hole = vec![0u8; w * h];
    let sx = src.w as f32 / w as f32;
    let sy = src.h as f32 / h as f32;
    for y in 0..h {
        for x in 0..w {
            let fx = ((x as f32 + 0.5) * sx - 0.5).max(0.0);
            let fy = ((y as f32 + 0.5) * sy - 0.5).max(0.0);
            let x0 = fx as usize;
            let y0 = fy as usize;
            let x1 = (x0 + 1).min(src.w - 1);
            let y1 = (y0 + 1).min(src.h - 1);
            let tx = fx - x0 as f32;
            let ty = fy - y0 as f32;
            let p00 = src.px[y0 * src.w + x0];
            let p10 = src.px[y0 * src.w + x1];
            let p01 = src.px[y1 * src.w + x0];
            let p11 = src.px[y1 * src.w + x1];
            let mut out = [0.0f32; 3];
            for c in 0..3 {
                let a = p00[c] * (1.0 - tx) + p10[c] * tx;
                let b = p01[c] * (1.0 - tx) + p11[c] * tx;
                out[c] = a * (1.0 - ty) + b * ty;
            }
            px[y * w + x] = out;
            hole[y * w + x] = src.hole[y0 * src.w + x0];
        }
    }
    Level { w, h, px, hole }
}

fn fill_with_boundary_mean(l: &mut Level) {
    let mut acc = [0.0f64; 3];
    let mut n = 0.0f64;
    for i in 0..l.w * l.h {
        if l.hole[i] == 0 {
            acc[0] += l.px[i][0] as f64;
            acc[1] += l.px[i][1] as f64;
            acc[2] += l.px[i][2] as f64;
            n += 1.0;
        }
    }
    if n == 0.0 {
        return;
    }
    let mean = [
        (acc[0] / n) as f32,
        (acc[1] / n) as f32,
        (acc[2] / n) as f32,
    ];
    for i in 0..l.w * l.h {
        if l.hole[i] != 0 {
            l.px[i] = mean;
        }
    }
}

fn solve(l: &mut Level, iters: usize, seed: u32) {
    let (w, h) = (l.w, l.h);
    let mut rng = Rng::new(seed.wrapping_mul(2654435761));
    let mut nnf = vec![(-1i32, -1i32); w * h];
    let mut cost = vec![f32::MAX; w * h];

    // 随机初始化匹配（只在已知像素里挑）
    let mut known: Vec<usize> = Vec::new();
    for i in 0..w * h {
        if l.hole[i] == 0 {
            known.push(i);
        }
    }
    if known.is_empty() {
        return;
    }
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if l.hole[i] == 0 {
                continue;
            }
            let k = known[rng.below(known.len())];
            let (sx, sy) = ((k % w) as i32, (k / w) as i32);
            nnf[i] = (sx, sy);
            cost[i] = patch_cost(l, x as i32, y as i32, sx, sy, f32::MAX);
        }
    }

    // 洞的包围盒：投票只在这个范围内扫，避免为一条细线扫整张图
    let (mut hx0, mut hy0, mut hx1, mut hy1) = (w as i32, h as i32, -1i32, -1i32);
    for y in 0..h {
        for x in 0..w {
            if l.hole[y * w + x] != 0 {
                hx0 = hx0.min(x as i32);
                hy0 = hy0.min(y as i32);
                hx1 = hx1.max(x as i32);
                hy1 = hy1.max(y as i32);
            }
        }
    }
    let vx0 = (hx0 - PATCH_R).max(0);
    let vy0 = (hy0 - PATCH_R).max(0);
    let vx1 = (hx1 + PATCH_R + 1).min(w as i32);
    let vy1 = (hy1 + PATCH_R + 1).min(h as i32);

    let mut votes = vec![[0.0f32; 3]; w * h];
    let mut weight = vec![0.0f32; w * h];

    for it in 0..iters {
        let forward = it % 2 == 0;
        // ---- E 步：PatchMatch 传播 + 随机搜索 ----
        for yy in 0..h {
            for xx in 0..w {
                let y = if forward { yy } else { h - 1 - yy };
                let x = if forward { xx } else { w - 1 - xx };
                let i = y * w + x;
                if l.hole[i] == 0 {
                    continue;
                }
                let (mut bx, mut by) = nnf[i];
                let mut bc = cost[i];

                // 传播：抄邻居的匹配
                let neighbors: [(isize, isize); 2] = if forward {
                    [(-1, 0), (0, -1)]
                } else {
                    [(1, 0), (0, 1)]
                };
                for (dx, dy) in neighbors {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    let ni = (ny as usize) * w + (nx as usize);
                    if nnf[ni].0 < 0 {
                        continue;
                    }
                    // 邻居的匹配位置沿同方向平移，作为候选
                    let cx = nnf[ni].0 - dx as i32;
                    let cy = nnf[ni].1 - dy as i32;
                    if !in_src(cx, cy, w, h) {
                        continue;
                    }
                    let c = patch_cost(l, x as i32, y as i32, cx, cy, bc);
                    if c < bc {
                        bc = c;
                        bx = cx;
                        by = cy;
                    }
                }

                // 随机搜索：以当前最优为中心，半径指数收缩
                let mut radius = w.max(h) as i32;
                while radius > 1 {
                    let span = (radius * 2 + 1) as u32;
                    let rx = bx + (rng.next() % span) as i32 - radius;
                    let ry = by + (rng.next() % span) as i32 - radius;
                    if in_src(rx, ry, w, h) {
                        let c = patch_cost(l, x as i32, y as i32, rx, ry, bc);
                        if c < bc {
                            bc = c;
                            bx = rx;
                            by = ry;
                        }
                    }
                    radius /= 2;
                }

                nnf[i] = (bx, by);
                cost[i] = bc;
            }
        }

        // ---- M 步：按偏移投票重建洞内像素 ----
        for v in votes.iter_mut() {
            *v = [0.0; 3];
        }
        for v in weight.iter_mut() {
            *v = 0.0;
        }
        for y in vy0..vy1 {
            for x in vx0..vx1 {
                let (x, y) = (x as usize, y as usize);
                let i = y * w + x;
                if l.hole[i] == 0 {
                    continue;
                }
                let (sx, sy) = nnf[i];
                if sx < 0 {
                    continue;
                }
                for dy in -PATCH_R..=PATCH_R {
                    for dx in -PATCH_R..=PATCH_R {
                        let tx = x as i32 + dx;
                        let ty = y as i32 + dy;
                        if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                            continue;
                        }
                        let ti = (ty as usize) * w + (tx as usize);
                        if l.hole[ti] == 0 {
                            continue;
                        }
                        let px_ = sx + dx;
                        let py_ = sy + dy;
                        if px_ < 0 || py_ < 0 || px_ >= w as i32 || py_ >= h as i32 {
                            continue;
                        }
                        let si = (py_ as usize) * w + (px_ as usize);
                        let src = l.px[si];
                        // 高斯权重：离 patch 中心越近权重越大
                        let d2 = (dx * dx + dy * dy) as f32;
                        let wgt = (-d2 / (2.0 * (PATCH_R as f32) * (PATCH_R as f32))).exp();
                        votes[ti][0] += src[0] * wgt;
                        votes[ti][1] += src[1] * wgt;
                        votes[ti][2] += src[2] * wgt;
                        weight[ti] += wgt;
                    }
                }
            }
        }
        for i in 0..w * h {
            if l.hole[i] != 0 && weight[i] > 0.0 {
                l.px[i] = [
                    votes[i][0] / weight[i],
                    votes[i][1] / weight[i],
                    votes[i][2] / weight[i],
                ];
            }
        }
    }
}

fn in_src(x: i32, y: i32, w: usize, h: usize) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < h as i32
}

/// 两个 patch 的均方差（按偏移逐像素比较，越界部分跳过）
/// best 用来提前收手：一旦超过当前最优就没必要算完
fn patch_cost(l: &Level, ax: i32, ay: i32, bx: i32, by: i32, best: f32) -> f32 {
    let w = l.w as i32;
    let h = l.h as i32;
    let mut sum = 0.0f32;
    let mut n = 0.0f32;
    for dy in -PATCH_R..=PATCH_R {
        let ay_ = ay + dy;
        let by_ = by + dy;
        if ay_ < 0 || by_ < 0 || ay_ >= h || by_ >= h {
            continue;
        }
        for dx in -PATCH_R..=PATCH_R {
            let ax_ = ax + dx;
            let bx_ = bx + dx;
            if ax_ < 0 || bx_ < 0 || ax_ >= w || bx_ >= w {
                continue;
            }
            let a = l.px[(ay_ * w + ax_) as usize];
            let b = l.px[(by_ * w + bx_) as usize];
            sum += (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
            n += 3.0;
        }
        // 逐行检查，够差了就提前退出
        if n > 0.0 && best < f32::MAX && sum / n >= best {
            return f32::MAX;
        }
    }
    if n == 0.0 {
        f32::MAX
    } else {
        sum / n
    }
}
