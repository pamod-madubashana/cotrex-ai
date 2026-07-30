# Download Qwen2.5-0.5B-Instruct GGUF for integration tests
#
# Usage:
#   .\download-model.ps1
#
# Requires: PowerShell 5.1+ (Windows built-in)

$modelUrl = "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf"
$outFile = Join-Path $PSScriptRoot "qwen2.5-0.5b-instruct-q4_k_m.gguf"

if (Test-Path $outFile) {
    Write-Host "Model already exists: $outFile"
    exit 0
}

Write-Host "Downloading Qwen2.5-0.5B-Instruct GGUF (~390MB)..."
Write-Host "From: $modelUrl"
Write-Host "To:   $outFile"
Write-Host ""

Invoke-WebRequest -Uri $modelUrl -OutFile $outFile

Write-Host ""
Write-Host "Downloaded successfully."
