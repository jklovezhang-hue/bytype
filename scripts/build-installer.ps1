#requires -version 5
# ByType 发布构建:产出当前用户 NSIS 安装包(target/release/bundle/nsis/*-setup.exe)。
# 用法(任意目录均可):  powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
# 注意:字符串字面量一律用 ASCII —— PowerShell 5.1 按 ANSI 码页读无 BOM 脚本,
#       字符串里的非 ASCII 会乱码并破坏解析;中文只放注释(到行尾结束,不影响解析)。
$ErrorActionPreference = "Stop"

# 切到仓库根(本脚本在 scripts/ 下,根 = 上一级)
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

# 构建环境:cargo / libclang 不在默认 PATH;crt-static 仅本次发布生效(不写 .cargo/config,避免污染 dev/test)
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:RUSTFLAGS = "-C target-feature=+crt-static"

# 步骤 1:release 构建。sherpa-rs 把引擎 DLL 产到 target/release/;bytype.exe 静态链接 CRT(缺 VC++ 也能启动)
Write-Host "[1/3] cargo build --release ..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit=$LASTEXITCODE)" }

# 步骤 2:拷引擎 DLL 到 src-tauri/runtime/(bundle.resources 的稳定来源;多带无害)
Write-Host "[2/3] copy engine DLLs -> src-tauri/runtime/ ..." -ForegroundColor Cyan
$runtime = Join-Path $repo "src-tauri\runtime"
New-Item -ItemType Directory -Force -Path $runtime | Out-Null
$src = Join-Path $repo "target\release"
$patterns = @("onnxruntime*.dll", "sherpa-onnx*.dll", "cargs.dll")
$copied = 0
foreach ($p in $patterns) {
    foreach ($f in (Get-ChildItem -Path $src -Filter $p -ErrorAction SilentlyContinue)) {
        Copy-Item $f.FullName -Destination $runtime -Force
        $copied++
    }
}
if ($copied -eq 0) { throw "no engine DLL found in target/release; cannot bundle" }
Write-Host "    copied $copied DLL(s)" -ForegroundColor Green

# 同时拷会议模型(VAD + 分人,共约 33MB)到 runtime/models/;bundle.resources 收为 ./models/。
# SenseVoice(约 228MB)不打包,仍由首启向导下载/导入。
$rtModels = Join-Path $runtime "models"
New-Item -ItemType Directory -Force -Path $rtModels | Out-Null
$models = @("silero_vad.onnx", "segmentation.onnx", "speaker_embedding.onnx")
$mcopied = 0
foreach ($m in $models) {
    $mp = Join-Path $repo "models\$m"
    if (Test-Path $mp) { Copy-Item $mp -Destination $rtModels -Force; $mcopied++ }
}
if ($mcopied -ne $models.Count) { throw "meeting models missing in models/; need silero_vad/segmentation/speaker_embedding .onnx" }
Write-Host "    copied $mcopied meeting model(s)" -ForegroundColor Green

# 步骤 3:打包。RUSTFLAGS 不变 → 此处 cargo 构建命中步骤 1 缓存(增量空转),打包器收 runtime/*.dll
Write-Host "[3/3] npm run tauri build ..." -ForegroundColor Cyan
npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "tauri build failed (exit=$LASTEXITCODE)" }

$setup = Get-ChildItem -Path (Join-Path $repo "target\release\bundle\nsis") -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
         Select-Object -First 1
if (-not $setup) { throw "NSIS installer not found" }
Write-Host "DONE installer: $($setup.FullName)" -ForegroundColor Green
