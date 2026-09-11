param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$ArchiveName
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Stage = Join-Path $Root "target\package-$Target"
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Path $Stage | Out-Null
Copy-Item (Join-Path $Root "target\$Target\release\rdr2-xbox-save-converter.exe") $Stage
Copy-Item (Join-Path $Root "README.md") $Stage
Copy-Item (Join-Path $Root "keys.example.toml") $Stage
Copy-Item (Join-Path $Root "LICENSE") $Stage
Compress-Archive -Path "$Stage\*" -DestinationPath (Join-Path $Root $ArchiveName) -Force

