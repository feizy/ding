param(
    [Parameter(Mandatory = $true)]
    [string]$InputPath,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    [int]$Fps = 12,
    [int]$Width = 900
)

$ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
if (-not $ffmpeg) {
    Write-Error "ffmpeg was not found in PATH. Install ffmpeg or use ScreenToGif to export GIF directly."
    exit 1
}

$palette = [System.IO.Path]::ChangeExtension([System.IO.Path]::GetTempFileName(), ".png")

try {
    & $ffmpeg.Source -y -i $InputPath `
        -vf "fps=$Fps,scale=$Width:-1:flags=lanczos,palettegen" `
        $palette

    if ($LASTEXITCODE -ne 0) {
        throw "ffmpeg palette generation failed with exit code $LASTEXITCODE"
    }

    & $ffmpeg.Source -y -i $InputPath -i $palette `
        -filter_complex "fps=$Fps,scale=$Width:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5" `
        $OutputPath

    if ($LASTEXITCODE -ne 0) {
        throw "ffmpeg GIF conversion failed with exit code $LASTEXITCODE"
    }
}
finally {
    if (Test-Path $palette) {
        Remove-Item -Force $palette
    }
}

Write-Host "Wrote $OutputPath"
