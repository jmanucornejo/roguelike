param(
    [string]$SpritesRoot = (Join-Path $PSScriptRoot '..\assets\spritesheets'),
    [double]$HeadHeightRatio = 0.38,
    [int]$HeadRadius = 25,
    [int]$NeckRadius = 15,
    [int]$NeckDepth = 7
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = (Resolve-Path -LiteralPath $SpritesRoot).Path
$characters = Get-ChildItem -LiteralPath $root -Directory |
    Where-Object {
        Test-Path -LiteralPath (Join-Path $_.FullName ($_.Name + '.png'))
    }

foreach ($character in $characters) {
    $role = $character.Name
    $sourcePath = Join-Path $character.FullName ($role + '.png')
    $outputPath = Join-Path $character.FullName ($role + '_headless.png')

    $source = New-Object System.Drawing.Bitmap($sourcePath)
    if ($source.Width -ne 1024 -or $source.Height -ne 1024) {
        $source.Dispose()
        throw "$sourcePath must be a 1024x1024 atlas."
    }

    $output = $source.Clone(
        (New-Object System.Drawing.Rectangle(0, 0, $source.Width, $source.Height)),
        $source.PixelFormat
    )

    for ($row = 0; $row -lt 8; $row++) {
        for ($column = 0; $column -lt 8; $column++) {
            $cellX = $column * 128
            $cellY = $row * 128
            $allMinY = 128
            $allMaxY = -1
            $rowWeights = New-Object int[] 128

            for ($localY = 0; $localY -lt 128; $localY++) {
                for ($localX = 0; $localX -lt 128; $localX++) {
                    if ($source.GetPixel($cellX + $localX, $cellY + $localY).A -gt 24) {
                        $rowWeights[$localY]++
                        if ($localY -lt $allMinY) { $allMinY = $localY }
                        if ($localY -gt $allMaxY) { $allMaxY = $localY }
                    }
                }
            }

            if ($allMaxY -lt $allMinY) {
                continue
            }

            # Ignore sparse row-boundary spill by selecting the longest
            # continuous band with at least four opaque pixels per scanline.
            $minY = $allMinY
            $maxY = $allMaxY
            $bestBandHeight = 0
            $localY = 0
            while ($localY -lt 128) {
                if ($rowWeights[$localY] -lt 4) {
                    $localY++
                    continue
                }

                $bandStart = $localY
                while ($localY -lt 128 -and $rowWeights[$localY] -ge 4) {
                    $localY++
                }
                $bandEnd = $localY - 1
                $bandHeight = $bandEnd - $bandStart + 1
                if ($bandHeight -gt $bestBandHeight) {
                    $bestBandHeight = $bandHeight
                    $minY = $bandStart
                    $maxY = $bandEnd
                }
            }

            $height = $maxY - $minY + 1
            # Find the widest/densest alpha cluster in the upper sprite band.
            # This follows profile-view heads while rejecting narrow, separate
            # staff and weapon silhouettes.
            $headBandEnd = [Math]::Min(
                127,
                $minY + [Math]::Max(18, [Math]::Round($height * 0.30))
            )
            $columnWeights = New-Object int[] 128
            for ($localX = 0; $localX -lt 128; $localX++) {
                for ($localY = $minY; $localY -le $headBandEnd; $localY++) {
                    if ($source.GetPixel(
                        $cellX + $localX,
                        $cellY + $localY
                    ).A -gt 24) {
                        $columnWeights[$localX]++
                    }
                }
            }

            $bestScore = -1
            $bestWeightedX = 64.0
            $bestWeight = 1.0
            $localX = 0
            while ($localX -lt 128) {
                if ($columnWeights[$localX] -eq 0) {
                    $localX++
                    continue
                }

                $segmentScore = 0
                $segmentWeightedX = 0
                $segmentWeight = 0
                while ($localX -lt 128 -and $columnWeights[$localX] -gt 0) {
                    $segmentScore += $columnWeights[$localX]
                    $segmentWeightedX += $localX * $columnWeights[$localX]
                    $segmentWeight += $columnWeights[$localX]
                    $localX++
                }

                if ($segmentScore -gt $bestScore) {
                    $bestScore = $segmentScore
                    $bestWeightedX = $segmentWeightedX
                    $bestWeight = $segmentWeight
                }
            }

            $centerX = [Math]::Round($bestWeightedX / $bestWeight)
            $centerX = [Math]::Max(32, [Math]::Min(96, $centerX))

            $cutY = [Math]::Min(
                112,
                $minY + [Math]::Max(26, [Math]::Round($height * $HeadHeightRatio))
            )
            $maskTop = [Math]::Max(0, $allMinY - 3)
            $maskBottom = [Math]::Min(127, $cutY + $NeckDepth)

            for ($localY = $maskTop; $localY -le $maskBottom; $localY++) {
                if ($localY -le $cutY) {
                    $radius = $HeadRadius
                } else {
                    $progress = ($localY - $cutY) / [Math]::Max(1, $NeckDepth)
                    $radius = [Math]::Round(
                        $HeadRadius - (($HeadRadius - $NeckRadius) * $progress)
                    )
                }

                $left = [Math]::Max(0, $centerX - $radius)
                $right = [Math]::Min(127, $centerX + $radius)
                for ($localX = $left; $localX -le $right; $localX++) {
                    $output.SetPixel(
                        $cellX + $localX,
                        $cellY + $localY,
                        [System.Drawing.Color]::FromArgb(0, 0, 0, 0)
                    )
                }
            }
        }
    }

    $output.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $output.Dispose()
    $source.Dispose()
    Write-Output $outputPath
}
