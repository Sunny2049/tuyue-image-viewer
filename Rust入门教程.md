# Rust 入门教程（通俗版）

> 拿「图阅」项目的代码当例子。尽量说人话，多打比方。学完能看懂 `app/src` 里的大部分代码。

## 目录

1. [Rust 到底是个啥](#1-rust-到底是个啥)
2. [装工具：Cargo](#2-装工具cargo)
3. [第一个程序](#3-第一个程序)
4. [变量、可变性和常量](#4-变量可变性和常量)
5. [类型和字符串](#5-类型和字符串)
6. [函数](#6-函数)
7. [判断和循环](#7-判断和循环)
8. [所有权、借用、生命周期](#8-所有权借用生命周期)
9. [结构体、枚举、match](#9-结构体枚举match)
10. [可能没有、可能失败：Option 和 Result](#10-可能没有可能失败option-和-result)
11. [模块和 pub](#11-模块和-pub)
12. [三种常用容器](#12-三种常用容器)
13. [迭代器和闭包](#13-迭代器和闭包)
14. [trait 和泛型](#14-trait-和泛型)
15. [测试和常用命令](#15-测试和常用命令)
16. [实战：读懂 assoc.rs](#16-实战读懂-assocrs)
17. [实战：读懂 ipc.rs](#17-实战读懂-ipcrs)
18. [新手最容易踩的坑](#18-新手最容易踩的坑)
19. [怎么学下去](#19-怎么学下去)

---

## 1. Rust 到底是个啥

一句话：**Rust 是个特别较真的语言，很多东西在你写代码的时候就检查好了，不等到运行时才出事。**

> 打个比方：别的语言像出门后才检查有没有带钥匙；Rust 像出门前站在门口逐样检查，没带就不让你出门。麻烦一点，但很少到了楼下才发现进不了门。

它有三个拿手好戏：

- **不会莫名其妙崩溃**：空指针、内存重复释放这类问题，编译器提前拦下来。
- **速度快**：没有垃圾回收器（GC）在后台扫来扫去，跑起来接近 C/C++。
- **打包省心**：一个 `cargo` 命令搞定下载依赖、编译、测试、出安装包。

和 JavaScript / Python 比，最大的不同：

| 事情 | JS / Python | Rust |
|---|---|---|
| 没有值 | `null` / `None` | 用 `Option` 装起来，必须打开看 |
| 出错了 | `try/catch` 抓 | 用 `Result` 当返回值传回来 |
| 变量 | 默认可改 | 默认不能改，要改得说一声 |
| 内存 | GC 自动收 | 谁创建谁负责，规则写死在编译期 |

「图阅」这个项目就是这么分工的：Rust 管窗口、读文件、调 OCR、写注册表；界面是 WebView2 里的 HTML/JS。最后打成一个约 6MB 的 exe。

---

## 2. 装工具：Cargo

### 安装

Windows 上最省事的是装 [rustup](https://rustup.rs/)：

```powershell
winget install Rustlang.Rustup
```

装完开个新终端，敲：

```powershell
rustc --version
cargo --version
```

能看到版本号就成了。

### Cargo 是干嘛的

> 打个比方：Cargo 既是**采购员**，也是**装修队**。你说要哪些材料（依赖），它去下载；你说开工，它负责编译、测试、打包。

常用命令，先混个脸熟：

```powershell
cargo new hello        # 新建一个项目
cargo build            # 编译（调试版）
cargo build --release  # 编译（正式版，体积小、跑得快）
cargo run              # 编译并运行
cargo test             # 跑测试
cargo fmt              # 自动排版
cargo clippy           # 挑刺，告诉你哪里写得不好
```

### Cargo.toml

这是项目的「材料清单」，长这样：

```toml
[package]
name = "tuyue"
version = "1.0.0"
edition = "2021"

[[bin]]
name = "图阅"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
```

- `edition = "2021"`：用哪一版语法，相当于「按 2021 年的规矩来」。
- `[dependencies]`：要用哪些现成的工具包（crate），编译时 Cargo 自动去下。
- `[[bin]]`：最终生成的可执行文件叫 `图阅.exe`。

`Cargo.lock` 会记下每个依赖的精确版本。把它提交到代码库，别人拉下来装的就是同一套版本，不会「我这能跑你那不能跑」。

---

## 3. 第一个程序

```rust
fn main() {
    println!("你好，图阅！");
}
```

拆开看：

- `fn` 是 function（函数）的缩写，`main` 是程序大门，从这里开始跑。
- `println!` 是打印。注意后面有个 `!`，意思是它不是普通函数，是**宏**——可以理解成「会自己变戏法的打印」，能接不同数量的参数。

运行：

```powershell
cargo run
```

打印时怎么把变量塞进去：

```rust
let name = "图阅";
let count = 3;

println!("{name} 处理了 {count} 张图");   // 直接把变量名写进花括号
println!("{} 处理了 {} 张图", name, count); // 按顺序填
println!("{count:>3}");                   // 占 3 格，靠右
println!("{:.2}", 3.14159);               // 保留两位小数
```

> 小技巧：`{}` 像填空题的横线，花括号里写变量名就更省事。

---

## 4. 变量、可变性和常量

### 变量默认不能改

```rust
let x = 5;
// x = 6;   // 这里会报错：x 不让改

let mut y = 5;
y = 6;      // 加了 mut 才能改
```

> 打个比方：`let` 是给盒子贴标签，默认标签上写着「封条，别动」。想以后还能换里面的东西，就加 `mut`（mutable，可变的），相当于把封条撕掉。

Rust 这样做是为了少出错。大部分变量其实创建后就不该再改，强制你多想一步。

### 换个标签（shadowing）

```rust
let s = "  42  ";
let s = s.trim();      // 同一个名字，重新贴个标签
let s: i32 = s.parse().unwrap();
```

这不算修改，而是**做了个新盒子，贴了同样的名字**，旧的被挡在后面。好处是类型都能变：前一个 `s` 是字符串，后一个 `s` 是数字。

> 比方：旧箱子还在，只是被新箱子挡住了，找东西时先看到新的。

### 常量

```rust
pub const PROGID: &str = "ImgTxt.Image";
```

> 打个比方：常量是**刻在石头上**的字，程序一编译就定死了，运行期间不能改，也没有自己的「位置」，用的时候直接抄一份过去。

对比一下：

| 写法 | 能不能改 | 什么时候定下来 | 常见用途 |
|---|---|---|---|
| `let` | 不能（除非 `mut`） | 运行时 | 普通变量 |
| `const` | 不能 | 编译时 | 固定配置、魔法数字 |
| `static` | 一般不能 | 编译时 | 全局唯一、需要固定地址 |

命名习惯：常量全大写加下划线，比如 `PROGID`、`IMAGE_EXTS`。看到全大写，基本就是常量。

---

## 5. 类型和字符串

### 类型就是盒子的规格

```rust
let a: i32 = -42;     // 整数，能表示负数
let b: u64 = 42;      // 整数，只能表示非负
let c: f64 = 3.14;    // 小数
let d: bool = true;   // 真 / 假
let e: char = '图';   // 单个字符
let f: usize = 0;     // 专门用来表示长度、下标的整数
```

> 比方：`i32` 是「32 格带正负号的抽屉」，`u64` 是「64 格不带负号的抽屉」。装错了编译器会提醒你。

平时最常打交道的是 `usize`。数组长度、下标都用它：

```rust
let v = vec![1, 2, 3];
let n: usize = v.len();   // 长度是 usize
```

### 两种字符串

这是新手最容易晕的地方，其实就两种：

| 类型 | 打个比方 | 能不能改 | 什么时候用 |
|---|---|---|---|
| `&str` | 图书馆的公共书，你只是**借来看** | 不能 | 函数参数、固定文字 |
| `String` | 自己买的笔记本，**归你** | 能 | 要拼接、要保存、要修改 |

```rust
let a: &str = "你好";                    // 借来的，指向程序里的固定文字
let b: String = String::from("你好");    // 自己拥有的，放在堆上
let c: &str = &b;                        // 从自己的笔记本上「借一页」看
```

> 打个比方：`&str` 是「书页的复印件地址」，`String` 是「你自己买的一本本子」。函数只想看看内容，收 `&str` 就够了，不用把整本本子送出去。

经验法则：

- **函数参数写 `&str`**：谁来都能用，`String` 也能自动借成 `&str`。
- **需要存起来、需要改，用 `String`**。

```rust
fn greet(name: &str) {
    println!("你好，{name}");
}

let mine = String::from("小明");
greet(&mine);    // 自己的笔记本也能借出去看
greet("小红");   // 借来的书直接给
```

常见操作：

```rust
let mut s = String::from("图阅");
s.push_str(" 真棒");        // 后面接一段文字
s.push('!');                // 后面接一个字符
let big = s.to_uppercase(); // 转大写
let n = s.len();            // 占多少字节（不是字数）
let count = s.chars().count(); // 有多少个字符
```

注意：Rust 的字符串是 UTF-8，一个汉字通常占 3 个字节。所以**不能用 `s[0]` 取第一个字**，要按字符取：

```rust
let first = "图阅".chars().next();   // 得到 Some('图')
```

---

## 6. 函数

> 打个比方：函数是**榨汁机**。你把水果（参数）放进去，它给你一杯果汁（返回值）。

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b        // 最后一行没分号，就是「榨出来的果汁」
}
```

几个规矩：

- 参数和返回值都要写明类型，`-> i32` 表示返回一个整数。
- **最后一行不加分号**，它就是返回值。
- 加了分号就变成「做完就扔」，返回值是空的 `()`。

```rust
fn add_wrong(a: i32, b: i32) -> i32 {
    a + b;      // 加了分号，结果被扔了，类型对不上，报错
}
```

提前返回：

```rust
fn abs(x: i32) -> i32 {
    if x < 0 {
        return -x;   // 提前交卷
    }
    x
}
```

想一次返回两个值，用元组（括号包起来的一组值）：

```rust
fn divmod(a: i32, b: i32) -> (i32, i32) {
    (a / b, a % b)
}

let (q, r) = divmod(7, 2);   // q = 3, r = 1
```

---

## 7. 判断和循环

### if：岔路口

```rust
let n = 7;

if n % 2 == 0 {
    println!("偶数");
} else if n > 0 {
    println!("正奇数");
} else {
    println!("其他");
}
```

`if` 还能直接拿来赋值，像「二选一」：

```rust
let label = if n > 0 { "正数" } else { "不是正数" };
```

### loop / while / for

```rust
let mut i = 0;
loop {
    i += 1;
    if i == 3 {
        break;     // 跳出循环
    }
}

while i > 0 {
    i -= 1;
}

for x in 0..3 {          // 0、1、2
    println!("{x}");
}
```

> 比方：`loop` 是「一直绕圈，直到有人喊停」；`while` 是「条件还成立就继续」；`for` 是「挨个点名」。

Rust 里最常用的是 `for`，遍历集合非常顺手：

```rust
let files = vec!["a.png", "b.jpg"];
for f in &files {          // 加 & 表示只借来看，不把集合吃掉
    println!("{f}");
}
```

---

## 8. 所有权、借用、生命周期

这是 Rust 最特别的地方，也是很多人第一次卡住的地方。别急，先记三个生活场景。

### 场景一：一支笔只有一个人拿着（所有权）

> 比方：一支笔同一时间只能在一个人的手里。你把笔递给别人，你自己手里就没了。

Rust 里值也一样：

```rust
let a = String::from("你好");
let b = a;          // 把「本子」交给 b，a 手里就没了
// println!("{a}"); // 报错：a 已经交出去了
println!("{b}");    // b 可以用
```

这叫**移动**（move）。好处是绝不会出现「两个人同时以为本子归自己，最后抢着撕」的情况。

如果不想交出去，而是想复印一份：

```rust
let a = String::from("你好");
let b = a.clone();   // 复印一本，两个人都有
```

数字、真假、单个字符这类小东西，复制起来很便宜，Rust 直接复制，不搞移动：

```rust
let x = 5;
let y = x;      // x 还能用
```

### 场景二：借书看（借用）

> 比方：朋友只想翻翻你的书，不用把书送给他。看完还你，书还是你的。

```rust
fn len(s: &str) -> usize {
    s.len()
}

let s = String::from("abc");
println!("{}", len(&s));   // 只是借出去看看
println!("{s}");           // 看完还是我的
```

`&s` 前面的 `&` 就是「借」的意思，叫**引用**。

如果朋友要帮你改几笔：

```rust
fn add_suffix(s: &mut String) {
    s.push_str("!");
}

let mut s = String::from("hi");
add_suffix(&mut s);   // 借出去改
```

### 场景三：看书时别人不能抢（借用规则）

同一时间，对同一个东西：

- 可以**很多人同时看**（多个 `&`）；
- **或者**只有**一个人能改**（一个 `&mut`）；
- 又看又改，不行。

```rust
let mut v = vec![1, 2, 3];
let first = &v[0];     // 有人在看第一页
// v.push(4);          // 报错：人家还在看，你不能改
println!("{first}");
```

> 比方：图书馆里一本书可以很多人同时看（复印本），但要是有人正在上面批注，别人就不能同时看，不然看到半截字变了。

这条规则看起来烦，其实是 Rust 能保证多线程安全、不会莫名其妙数据错乱的底气。

### 生命周期：借条上的归还时间

生命周期就是「这个引用能活多久」。大部分时候编译器自己会算，不用你写。

只有一种情况要你帮忙：函数返回一个引用，而编译器分不清它是从哪个参数借的。

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}
```

`'a` 读作「a 的生命周期」，意思是：返回的引用，最多活到 a 和 b 里较短的那个结束。

> 比方：借条上写「最晚归还日期按两个人里最早到期的算」，这样绝不会出现书还在别人手里、借条却过期了。

新手建议：**能返回 `String` 就别返回引用**，先绕开生命周期，等熟了再回来。

---

## 9. 结构体、枚举、match

### 结构体：一张登记表

> 比方：结构体是一张表格，把相关的几项信息装在一起。

```rust
#[derive(Debug, Clone)]
struct ImageInfo {
    name: String,
    size: u64,
}

let info = ImageInfo {
    name: String::from("a.png"),
    size: 2048,
};

println!("{:?}", info);      // 打印整张表
println!("{}", info.name);   // 取其中一格
```

`#[derive(...)]` 是让编译器「顺手帮你生成」一些功能：

- `Debug`：能用 `{:?}` 打印出来看。
- `Clone`：能 `.clone()` 复印。
- `serde::Serialize`：能变成 JSON 发给前端。

给结构体加方法：

```rust
impl ImageInfo {
    fn new(name: &str, size: u64) -> Self {
        Self { name: name.to_string(), size }
    }

    fn size_kb(&self) -> u64 {
        self.size / 1024
    }
}
```

`&self` 表示「只读地看看自己」，`&mut self` 表示「可以改自己」，没有 `self` 就是「不需要某个具体对象」的工具函数。

### 枚举：一份菜单

> 比方：枚举是一份固定选项的菜单，只能从这几样里选。

```rust
enum Source {
    UserChoice,
    Hkcu,
    Hklm,
    None,
}
```

枚举还能顺手带点东西：

```rust
enum AppEvent {
    Ipc(serde_json::Value),
    IpcDone(i64, serde_json::Value),
    Drop(Vec<String>),
    OpenPath(String),
}
```

> 比方：菜单上「套餐 A 带饮料，套餐 B 带玩具」。选项不同，附带的东西也不同。

### match：分拣员

> 比方：`match` 是快递站的分拣员，来一个包裹，按面单放进对应的格子，所有格子都要考虑到。

```rust
let src = Source::Hkcu;

match src {
    Source::UserChoice => println!("用户选的"),
    Source::Hkcu => println!("当前用户设置的"),
    Source::Hklm => println!("系统级设置的"),
    Source::None => println!("没设置"),
}
```

`match` 有个硬要求：**所有情况都要列出来**，漏一个编译不过。这样就不会出现「没考虑到的意外」。

只看一种情况时，用 `if let` 更省事：

```rust
if let Some(name) = maybe_name {
    println!("{name}");
}
```

---

## 10. 可能没有、可能失败：Option 和 Result

Rust 没有 `null`，也不抛异常。它用两个「盒子」来表达。

### Option：盲盒

> 比方：`Option` 是个盲盒，打开可能是 `Some(东西)`，也可能是 `None`（空的）。

```rust
let maybe: Option<&str> = Some("a.png");

match maybe {
    Some(name) => println!("有：{name}"),
    None => println!("没有"),
}
```

读文件取扩展名这种「可能没有」的事，就用 `Option`：

```rust
fn ext_of(path: &str) -> Option<&str> {
    path.rsplit('.').next()
}
```

常用的小工具：

```rust
let v: Option<i32> = Some(5);
let a = v.unwrap_or(0);          // 有就拿出来，没有就给 0
let b = v.map(|x| x * 2);        // 有就加工一下
let c = v.filter(|x| *x > 3);    // 满足条件才留下
```

本项目 `assoc.rs` 读备份文件那段，就是「一条龙兜底」：

```rust
std::fs::read_to_string(backup_path())
    .ok()                                           // 读失败？变 None
    .and_then(|s| serde_json::from_str(&s).ok())    // 解析失败？也变 None
    .unwrap_or_default()                            // 最后还没有？给个空的
```

> 比方：去柜子里找备份，找不到就用空本子，反正不能让程序摔跤。

### Result：快递

> 比方：`Result` 是一次快递，要么 `Ok(包裹)` 签收成功，要么 `Err(原因)` 退回。

```rust
fn read_file(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

match read_file("a.txt") {
    Ok(text) => println!("{text}"),
    Err(e) => println!("读失败：{e}"),
}
```

### `?`：自动转交

在一个「会返回 Result」的函数里，`?` 的意思是：

> 「这一步要是失败了，我就不往下做了，直接把原因交给上一层。」

```rust
fn load(path: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取失败：{e}"))?;
    Ok(text)
}
```

### unwrap：硬拆盲盒

```rust
let n: i32 = "42".parse().unwrap();
```

`unwrap()` 的意思是「我确定里面有东西，直接拆」。但如果里面是空的或者失败了，**程序会当场崩溃**。

> 比方：蒙着眼睛拆盲盒，拆到空的就炸。自己写小练习可以用，给用户用的地方别乱拆。

原则：文件、网络、用户输入这些「外界给的东西」都可能出问题，要用 `match`、`?` 或 `unwrap_or` 稳稳接住。

---

## 11. 模块和 pub

> 比方：模块就是**抽屉**。每个抽屉放一类东西，`pub` 是给抽屉贴「公用」标签，没贴的就是私人抽屉。

`main.rs` 顶部：

```rust
mod assoc;
mod assets;
mod ipc;
mod ocr;
mod shot;
mod single;
mod util;
```

每写一个 `mod 名字;`，Rust 就去找 `src/名字.rs`，把它当成一个抽屉。

可见性：

```rust
pub const PROGID: &str = "ImgTxt.Image";   // 贴了「公用」，别的文件能用

fn hkcu() -> RegKey { /* ... */ }          // 没贴，只有 assoc.rs 自己用
```

`use` 是「抄近路」：

```rust
use std::collections::BTreeMap;
```

> 比方：每次都要写「三楼左转第二个房间的 BTreeMap」太累，`use` 就是在门口贴个牌子，以后直接喊名字。

路径里的几个前缀：

- `crate::`：从项目大门口开始找，比如 `crate::util::app_data_dir()`。
- `super::`：去上一层抽屉找。
- `self::`：就在当前抽屉里。

---

## 12. 三种常用容器

### Vec：能伸缩的收纳箱

```rust
let mut v: Vec<i32> = Vec::new();
v.push(1);          // 往里放
v.push(2);

let v2 = vec![1, 2, 3];   // 一步到位
let first = v2[0];         // 按下标拿
let maybe = v2.get(10);    // 越界不会崩，返回 None

for x in &v2 {
    println!("{x}");
}
```

> 比方：`Vec` 是一排能自动加长缩短的格子，按顺序放东西，也能按编号取。

### HashMap：快递柜

```rust
use std::collections::HashMap;

let mut m = HashMap::new();
m.insert("png", 3);
m.insert("jpg", 5);

if let Some(n) = m.get("png") {
    println!("{n}");
}
```

> 比方：`HashMap` 是快递柜，用取件码（键）直接找到格子（值），不管你放进去的先后顺序。找得快，但格子的排列顺序是乱的。

### BTreeMap：按拼音排好的字典

```rust
use std::collections::BTreeMap;

let mut m: BTreeMap<String, String> = BTreeMap::new();
m.insert(".png".to_string(), "ImgTxt.Image".to_string());
m.insert(".jpg".to_string(), "ImgTxt.Image".to_string());

for (k, v) in &m {
    println!("{k} -> {v}");   // 按键的字母顺序输出
}
```

> 比方：`BTreeMap` 是本按拼音排好序的字典。找得稍慢一点点，但翻起来永远井井有条。

本项目用它存备份，就是为了让写出来的 `assoc-backup.json` 每次顺序都一样，方便对比和查看。

怎么选：

- 只查、不在乎顺序 → `HashMap`
- 要按顺序遍历、要结果稳定 → `BTreeMap`
- 只是一串东西 → `Vec`

---

## 13. 迭代器和闭包

### 迭代器：流水线

> 比方：迭代器是一条流水线，原料从一头进去，经过一道道工序，最后出来成品。

```rust
let files = vec!["a.png", "b.jpg", "c.png"];

let pngs: Vec<&&str> = files.iter()
    .filter(|f| f.ends_with(".png"))   // 只留 png
    .collect();                         // 收进箱子

let upper: Vec<String> = files.iter()
    .map(|f| f.to_uppercase())          // 每个都转大写
    .collect();

let count = files.iter()
    .filter(|f| f.ends_with(".png"))
    .count();                           // 数一数有几个
```

常用工序：

| 工序 | 大白话 |
|---|---|
| `map` | 每个都加工一下 |
| `filter` | 只要满足条件的 |
| `find` | 找第一个满足条件的 |
| `any` / `all` | 有没有 / 是不是全部 |
| `take(n)` / `skip(n)` | 取前 n 个 / 跳过前 n 个 |
| `enumerate` | 顺便带上编号 |
| `collect` | 收起来变成 `Vec` 等 |
| `sum` / `count` | 求和 / 计数 |

> 好处：不用手写「先建空箱子，再循环，再往里塞」，一眼能看出在干什么。

### 闭包：随身小口袋

> 比方：闭包是个**随身小口袋**，能把身边的东西装进去，用的时候直接掏出来。

```rust
let factor = 2;
let double = |x: i32| x * factor;   // 把外面的 factor 装进口袋
println!("{}", double(3));          // 6
```

本项目 `assoc.rs` 里用闭包把「建注册表键 + 写值」包起来，省得写五遍：

```rust
let mut set = |path: &str, name: &str, val: &str| {
    if let Err(e) = cu
        .create_subkey(path)
        .and_then(|(k, _)| k.set_value(name, &val.to_string()))
    {
        errs.push(format!("{path}[{name}]：{e}"));
    }
};
```

> 比方：这个闭包像一个「写入小工」，你告诉它写哪、写什么，它自己会记账（把错误塞进 `errs`）。

注意：闭包已经把 `errs` 拿在手里了，在它还没用完之前，别的地方不能同时改 `errs`，否则编译器会拦你。

---

## 14. trait 和泛型

### trait：技能证书 / 插座标准

> 比方：`trait` 是一张「技能证书」或者一个「插座标准」。只要你会这项技能、符合这个标准，就能插上去用。

```rust
trait Describable {
    fn describe(&self) -> String;
}

struct ImageInfo {
    name: String,
    size: u64,
}

impl Describable for ImageInfo {
    fn describe(&self) -> String {
        format!("{}（{} 字节）", self.name, self.size)
    }
}
```

> 比方：`Describable` 是「会自我介绍」证书，`ImageInfo` 去考了这张证，于是它就能被要求「介绍一下自己」。

你可能没注意，其实一直在用标准库的 trait：

- `Debug`：能让 `{:?}` 打印
- `Clone`：能 `.clone()`
- `Display`：能让 `{}` 打印
- `Serialize`：能转 JSON

### 泛型：万能模具

```rust
fn largest<T: PartialOrd>(list: &[T]) -> &T {
    let mut max = &list[0];
    for item in list {
        if item > max {
            max = item;
        }
    }
    max
}
```

> 比方：泛型是「万能模具」，同一套逻辑，倒进不同材料（数字、字符串）都能用。`<T: PartialOrd>` 是模具上的说明：「材料必须能比大小」。

`Option<T>`、`Result<T, E>`、`Vec<T>` 里的 `<T>` 都是这个意思：先留个空位，用的时候再填具体类型。

---

## 15. 测试和常用命令

### 测试：质检员

> 比方：测试就是出厂前的质检员，自动帮你把每个功能试一遍。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_works() {
        assert_eq!(add(2, 3), 5);
    }
}
```

- `#[cfg(test)]`：这段只在测试时编译，不会混进正式程序。
- `#[test]`：标出这是一个测试。
- `assert_eq!`：相等就通过，不等就报错并告诉你差在哪。

```powershell
cargo test                  # 跑全部测试
cargo test add_works        # 只跑名字里有 add_works 的
cargo test -- --nocapture   # 顺便显示测试里的打印
```

### 日常工具

```powershell
cargo fmt       # 自动排版，别手调空格了
cargo clippy    # 挑刺师傅，指出写得别扭的地方
cargo check     # 只做检查，不生成程序，比 build 快
```

项目里 `Cargo.toml` 的 release 设置：

```toml
[profile.release]
opt-level = "s"    # 优先减小体积
lto = true         # 链接时再优化一遍
codegen-units = 1  # 优化更彻底，编译更慢
panic = "abort"    # 出错直接退出，不保留展开信息
strip = true       # 去掉调试符号
```

> 比方：这是一套「压缩打包」设置，牺牲一点编译时间，换更小的 exe。所以最终只有 6MB 左右。

---

## 16. 实战：读懂 assoc.rs

这个文件干的事：**把「图阅」登记成 Windows 的看图程序，并支持撤销。**

建议按这个顺序看：

1. 顶部常量：`PROGID`、`LABEL`、`EXTS`——知道要动哪些东西。
2. 两个结构体：`Item`、`Status`——知道结果长什么样。
3. 几个小函数：`hkcu`、`hklm`、`read_default`、`read_str`——读注册表的工具。
4. `status()`：只查不改，最好懂。
5. `register()`：真正写入，用到闭包和循环。
6. `unregister()`：和写入对着干，负责还原。
7. 最后两个函数：打开系统设置、通知资源管理器刷新。

几处值得学：

**不区分大小写比较**

```rust
let ours = prog_id.eq_ignore_ascii_case(PROGID);
```

> 比方：检查名字是不是自己人，不管大写小写，`PNG` 和 `png` 都算。

**一连串「失败了就降级」**

```rust
root.open_subkey_with_flags(path, KEY_READ)
    .ok()
    .and_then(|k| k.get_value::<String, _>("").ok())
    .filter(|v| !v.is_empty())
```

> 打不开？算了。读不到？算了。读到空字符串？也当没有。全程不崩溃。

**动手前先留后路**

```rust
if cur.eq_ignore_ascii_case(PROGID) || cur.is_empty() {
    // 只有当前确实是我们的，或者本来是空的，才还原
}
```

> 比方：还原之前先看看现在还是不是自己改的那个，别人后来改过就别乱动，免得把用户的设置弄丢。

**unsafe：拆掉护栏**

```rust
unsafe {
    let r = ShellExecuteW(None, &verb, &target, None, None, SW_SHOWNORMAL);
    if (r.0 as isize) <= 32 {
        // Win32 的规矩：返回值小于等于 32 就是失败
    }
}
```

> 比方：`unsafe` 是拆掉编译器的护栏去调系统接口。不是一定危险，而是**「这里编译器帮不了你，你自己看着办」**。本项目只在调 Windows 接口时用。

---

## 17. 实战：读懂 ipc.rs

前端发请求，长这样：

```js
window.api.invoke('readFile', { path: 'C:/a.png' })
```

Rust 这边在 `ipc.rs` 里按命令名分活：

```rust
pub fn dispatch(cmd: &str, args: &Value) -> Value {
    match cmd {
        "readFile" => {
            let p = arg_str(args, "path");
            match std::fs::read(&p) {
                Ok(bytes) => json!(base64::engine::general_purpose::STANDARD.encode(bytes)),
                Err(e) => json!({ "error": format!("读取失败：{e}") }),
            }
        }
        _ => json!({ "error": format!("未知命令：{cmd}") }),
    }
}
```

> 比方：`dispatch` 是个前台接待，看客户说哪句话，就转到对应的窗口。

值得学的几个习惯：

- **用 `match` 做路由**：一句命令对应一件事，清楚。
- **错误也当普通数据返回**：出错就返回 `{ "error": ... }`，让前端统一处理，而不是让程序崩掉。
- **辅助函数兜底**：

```rust
fn arg_str(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}
```

> 比方：从快递单上找某一栏，找不到就给张空白纸，绝不因为找不到就发火。

再看 `main.rs` 的模块划分：

```rust
mod assoc;   // 注册表关联
mod assets;  // 内嵌前端资源
mod ipc;     // 命令分发
mod ocr;     // 文字识别
mod shot;    // 截图自检
mod single;  // 单实例
mod util;    // 工具函数
```

> 比方：一个抽屉放一类东西，`main.rs` 只负责把抽屉摆好、把程序启动起来。这是 Rust 项目很常见的组织方式。

---

## 18. 新手最容易踩的坑

### 1. 字符串类型对不上

```rust
fn takes_str(s: &str) {}

let owned = String::from("a");
takes_str(&owned);    // 对：借出去
// takes_str(owned);  // 错：类型不一样
```

> 记住：**参数要 `&str`，自己的东西用 `String`。**

### 2. 正看着呢，又要改

```rust
let mut v = vec![1, 2, 3];
let first = &v[0];   // 有人正在看
v.push(4);           // 错：不能同时改
```

> 要么看完再改，要么先把值抄出来：`let first = v[0];`。

### 3. 循环把东西吃掉了

```rust
let names = vec![String::from("a"), String::from("b")];

for n in names {        // 直接遍历，names 被吃掉了
    println!("{n}");
}
// println!("{:?}", names);  // 错：已经没了
```

> 只想看看就别吃：`for n in &names { ... }`。

### 4. 拿字符串按下标取

```rust
let s = String::from("图阅");
// let c = s[0];              // 错
let c = s.chars().next();     // 对：得到 Some('图')
```

> 因为一个汉字占好几个字节，按下标可能劈开半个字。要按「字」取就用 `.chars()`。

### 5. 忘了 mut

```rust
let v = Vec::new();
// v.push(1);   // 错：盒子封着呢

let mut v = Vec::new();
v.push(1);       // 对
```

### 6. 最后一行多了分号

```rust
fn f() -> i32 {
    5      // 对：交出 5
}

fn g() -> i32 {
    5;     // 错：交出去的是「啥也没有」
}
```

> 分号像「句号」。最后一句加了句号，就表示「说完了，不交东西」。

### 7. 到处 unwrap

```rust
let n: i32 = input.parse().unwrap();   // 用户乱填就崩
```

> 改成：

```rust
let n: i32 = match input.parse() {
    Ok(v) => v,
    Err(_) => {
        println!("请输入数字");
        return;
    }
};
```

### 8. 整数算过头

debug 模式下溢出会直接报错，release 模式会绕回去变成很小的数。要明确行为就用：

```rust
let a = x.wrapping_add(y);    // 绕回去
let b = x.checked_add(y);     // 溢出返回 None
let c = x.saturating_add(y);  // 停在最大值
```

> 比方：里程表到 99999 之后，是归零、还是停住、还是提示「到顶了」，三种处理方式自己选。

### 9. 闭包把变量占着

闭包可变借用了 `errs`，在闭包还活着的时候，别的地方不能同时改 `errs`。把闭包放进一对 `{}` 里，用完就结束，问题就没了。

### 10. 编译慢

Rust 编译确实不快。日常改代码用 `cargo check` 做检查，真要跑起来再 `cargo run`，能省不少等待。

---

## 19. 怎么学下去

### 第一步：先把语法摸熟

变量、类型、函数、判断循环、结构体、枚举、`match`、`Option`、`Result`。

> 目标：能写个命令行小程序，比如数一数某个文件夹里有多少张图片。

### 第二步：啃下所有权

移动、复制、借用、借用规则、生命周期入门。

> 目标：能看懂并小改 `assoc.rs` 这种代码。这一步过了，Rust 就算入门了。

### 第三步：学会工程化

模块、`pub`、Cargo 依赖、测试、`clippy`、泛型和 trait。

> 目标：能给「图阅」新增一个 IPC 命令。

### 第四步：挑一个方向深入

- 桌面程序：`tao` + `wry`（就是本项目）
- 网络服务：`axum`、`actix-web`
- 异步：`tokio`
- 数据处理：`serde`、`serde_json`

### 好用的免费资料

- 官方书（有中文）：<https://doc.rust-lang.org/book/>
- 例子驱动教程：<https://doc.rust-lang.org/rust-by-example/>
- 标准库查询：<https://doc.rust-lang.org/std/>
- 闯关练习：<https://github.com/rust-lang/rustlings>

### 练手小题

1. 在 `ipc.rs` 加一个 `"imageCount"` 命令，返回某个文件夹里的图片数量。
2. 给 `status()` 写一个单元测试。
3. 用 `clap` 做一个独立的小命令行工具，练参数解析。
4. 把 `util.rs` 里某个函数改成返回 `Result`，再用 `?` 重构调用它的地方。

---

## 附：本项目速查

| 想学什么 | 去哪里看 |
|---|---|
| 程序入口、模块声明 | [main.rs](file:///e:/Indie%20Maker/img2txt/app/src/main.rs) |
| 常量 `pub const` | [assoc.rs L13-L19](file:///e:/Indie%20Maker/img2txt/app/src/assoc.rs#L13-L19) |
| `BTreeMap` 备份 | [assoc.rs L92-L107](file:///e:/Indie%20Maker/img2txt/app/src/assoc.rs#L92-L107) |
| 闭包收集错误 | [assoc.rs L121-L128](file:///e:/Indie%20Maker/img2txt/app/src/assoc.rs#L121-L128) |
| `Option` 一条龙兜底 | [ipc.rs L6-L11](file:///e:/Indie%20Maker/img2txt/app/src/ipc.rs#L6-L11) |
| `match` 命令分发 | [ipc.rs L13-L33](file:///e:/Indie%20Maker/img2txt/app/src/ipc.rs#L13-L33) |
| `Result` 错误处理 | [assoc.rs L109-L169](file:///e:/Indie%20Maker/img2txt/app/src/assoc.rs#L109-L169) |
| `unsafe` 调 Windows 接口 | [assoc.rs L210-L231](file:///e:/Indie%20Maker/img2txt/app/src/assoc.rs#L210-L231) |
| 枚举带数据 | [main.rs L29-L34](file:///e:/Indie%20Maker/img2txt/app/src/main.rs#L29-L34) |
| 内嵌前端资源 | [assets.rs](file:///e:/Indie%20Maker/img2txt/app/src/assets.rs) |

读到这里，再回去看 `assoc.rs`，应该能一边读一边说出「这句在干嘛」。
