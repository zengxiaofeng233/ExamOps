$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskSidecar = Join-Path $taskRoot 'python-sidecar'
$taskPython = Join-Path $taskSidecar 'venv/Scripts/python.exe'
if (-not (Test-Path -LiteralPath $taskPython)) {
    python -m venv (Join-Path $taskSidecar 'venv')
    if ($LASTEXITCODE -ne 0) { throw 'Python environment creation failed' }
}
# CPU and DirectML wheels share the onnxruntime module; never overlay both.
& $taskPython -c "import importlib.metadata as m; raise SystemExit(0 if any(d.metadata['Name'] == 'onnxruntime' for d in m.distributions()) else 1)"
if ($LASTEXITCODE -eq 0) {
    & $taskPython -m pip uninstall -y onnxruntime onnxruntime-directml
    if ($LASTEXITCODE -ne 0) { throw 'Conflicting inference runtime cleanup failed' }
}
& $taskPython -m pip install -r (Join-Path $taskSidecar 'requirements-build.txt')
if ($LASTEXITCODE -ne 0) { throw 'Sidecar dependencies installation failed' }
& $taskPython -m PyInstaller --noconfirm --onedir --name examops-parser --collect-all rapidocr --collect-all pypdfium2 --collect-all pypdfium2_raw --collect-all onnxruntime --hidden-import rapidocr.main --hidden-import rapidocr.inference_engine.onnxruntime --distpath (Join-Path $taskSidecar 'dist') --workpath (Join-Path $taskSidecar 'build') --specpath $taskSidecar (Join-Path $taskSidecar 'main.py')
if ($LASTEXITCODE -ne 0) { throw 'Sidecar packaging failed' }
$taskBinaries = Join-Path $taskRoot 'src-tauri/binaries'
New-Item -ItemType Directory -Force -Path $taskBinaries | Out-Null
# Start from a clean _internal so files from an older build are not shipped.
$taskInternal = Join-Path $taskBinaries '_internal'
if (Test-Path -LiteralPath $taskInternal) {
    $taskResolvedInternal = (Resolve-Path -LiteralPath $taskInternal).Path
    $taskResolvedBinaries = (Resolve-Path -LiteralPath $taskBinaries).Path
    if ($taskResolvedInternal -ne (Join-Path $taskResolvedBinaries '_internal')) { throw 'Unexpected bundle path' }
    Remove-Item -LiteralPath $taskResolvedInternal -Recurse -Force
}
Copy-Item -LiteralPath (Join-Path $taskSidecar 'dist/examops-parser/examops-parser.exe') -Destination (Join-Path $taskBinaries 'examops-parser-x86_64-pc-windows-msvc.exe') -Force
Copy-Item -LiteralPath (Join-Path $taskSidecar 'dist/examops-parser/_internal') -Destination $taskBinaries -Recurse -Force
