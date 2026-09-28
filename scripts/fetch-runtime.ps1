$ErrorActionPreference = "Stop"
$dest = Join-Path $PSScriptRoot "..\src-tauri\resources"
$tmp = Join-Path $env:TEMP "sorter-runtime"
New-Item -ItemType Directory -Force $dest, $tmp | Out-Null

$ort = "1.24.4"
$dml = "1.15.4"
Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime.directml/$ort/microsoft.ml.onnxruntime.directml.$ort.nupkg" -OutFile "$tmp\ort.zip"
Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.ai.directml/$dml/microsoft.ai.directml.$dml.nupkg" -OutFile "$tmp\dml.zip"
Expand-Archive "$tmp\ort.zip" "$tmp\ort" -Force
Expand-Archive "$tmp\dml.zip" "$tmp\dml" -Force
Copy-Item "$tmp\ort\runtimes\win-x64\native\onnxruntime.dll" $dest -Force
Copy-Item "$tmp\dml\bin\x64-win\DirectML.dll" $dest -Force
Invoke-WebRequest "https://huggingface.co/onnx-community/dinov2-small/resolve/main/onnx/model_fp16.onnx" -OutFile "$dest\dinov2-small-fp16.onnx"
Get-ChildItem $dest
