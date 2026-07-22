# 将 signing/ 签名配置应用到 gen/android（android init 之后执行）
$ErrorActionPreference = "Stop"
python (Join-Path $PSScriptRoot "apply_android_signing.py")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
