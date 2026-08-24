# CED ses etiketleme modellerini indirir (sherpa-onnx ONNX exportlari).
# Kullanim: .\fetch-models.ps1 [-Model ced-tiny|ced-mini|ced-small|ced-base]
param([string]$Model = "ced-base")
$ErrorActionPreference = "Stop"

$valid = @("ced-tiny", "ced-mini", "ced-small", "ced-base")
if ($valid -notcontains $Model) { throw "Bilinmeyen model: $Model ($($valid -join '|'))" }

$base = "https://huggingface.co/k2-fsa/sherpa-onnx-$Model-audio-tagging-2024-04-19/resolve/main"
$dir = Join-Path $PSScriptRoot "..\models\$Model"
New-Item -ItemType Directory -Force (Join-Path $dir "test_wavs") | Out-Null

function Fetch($rel) {
    $dest = Join-Path $dir $rel
    if (Test-Path $dest) { Write-Host "atlandi (var): $rel"; return }
    Write-Host "indiriliyor: $rel"
    Invoke-WebRequest -Uri "$base/$rel" -OutFile $dest
}

Fetch "model.onnx"
Fetch "model.int8.onnx"
Fetch "class_labels_indices.csv"
# Faz 1 dogrulama kapisi icin referans ses dosyalari (bkz. plan §10)
1..4 | ForEach-Object { Fetch "test_wavs/$_.wav" }

Write-Host "Tamam: $dir"
