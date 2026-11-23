$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$log = @()
$tmp = "$env:TEMP\msysdeps"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$target = "E:\Indie Maker\img2txt\tools\mingw\bin"

$list = (Invoke-WebRequest -Uri "https://repo.msys2.org/mingw/ucrt64/" -UseBasicParsing -TimeoutSec 60).Content
$log += "listing bytes: $($list.Length)"

$patterns = @(
  'mingw-w64-ucrt-x86_64-gettext-runtime-[0-9][^"<>]*?\.pkg\.tar\.zst',
  'mingw-w64-ucrt-x86_64-libiconv-[0-9][^"<>]*?\.pkg\.tar\.zst',
  'mingw-w64-ucrt-x86_64-zlib-[0-9][^"<>]*?\.pkg\.tar\.zst',
  'mingw-w64-ucrt-x86_64-zstd-[0-9][^"<>]*?\.pkg\.tar\.zst'
)

foreach ($p in $patterns) {
  $m = [regex]::Matches($list, $p)
  if ($m.Count -eq 0) { $log += "no match: $p"; continue }
  $name = ($m | ForEach-Object { $_.Value } | Select-Object -Unique | Select-Object -Last 1)
  $log += "pkg: $name"
  $zst = Join-Path $tmp ($name)
  try {
    Invoke-WebRequest -Uri ("https://repo.msys2.org/mingw/ucrt64/" + $name) -OutFile $zst -UseBasicParsing -TimeoutSec 300
  } catch { $log += "  download fail: $($_.Exception.Message)"; continue }
  $tarF = Join-Path $tmp (($name -replace '\.zst$', '') + ".tar")
  & node "E:\Indie Maker\img2txt\tools\unzst.js" $zst $tarF 2>&1 | Out-Null
  $outDir = Join-Path $tmp (($name -replace '\.pkg\.tar\.zst$', ''))
  New-Item -ItemType Directory -Force -Path $outDir | Out-Null
  Push-Location $outDir
  & "D:\Soft\Git\usr\bin\tar.exe" --force-local -xf $tarF 2>&1 | Out-Null
  Pop-Location
  $bins = Get-ChildItem (Join-Path $outDir "ucrt64\bin") -ErrorAction SilentlyContinue
  if ($bins) {
    $bins | Where-Object { $_.Extension -eq '.dll' } | ForEach-Object { Copy-Item $_.FullName $target -Force; $log += "  + " + $_.Name }
    $log += "  dll count: " + ($bins | Where-Object { $_.Extension -eq '.dll' } | Measure-Object).Count
  } else { $log += "  no ucrt64/bin" }
}

$log += "-- dlltool test:"
$env:Path = "$target;" + $env:Path
$t2 = "$env:TEMP\dlltest2"
New-Item -ItemType Directory -Force -Path $t2 | Out-Null
[System.IO.File]::WriteAllText("$t2\k.def", "LIBRARY kernel32.dll`r`nEXPORTS`r`nGetLastError`r`n", (New-Object System.Text.UTF8Encoding($false)))
$o = & "$target\dlltool.exe" -d "$t2\k.def" -l "$t2\k.lib" 2>&1 | Out-String
$log += "exit=$LASTEXITCODE out=$o"
$log += "lib: " + (Test-Path "$t2\k.lib") + " size=" + (Get-Item "$t2\k.lib" -ErrorAction SilentlyContinue).Length
[System.IO.File]::WriteAllText("$env:TEMP\msysdeps.log", ($log -join "`r`n"), (New-Object System.Text.UTF8Encoding($false)))
