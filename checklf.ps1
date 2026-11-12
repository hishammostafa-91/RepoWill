$files = @('gitignore-node','gitignore-go','gitignore-java','gitignore-dotnet','gitignore-python','gitignore.txt','header-0bsd.txt','header-0bsd-c.txt','header-0bsd-hash.txt','header-0bsd-html.txt','policy.toml')
foreach ($f in $files) {
    $p = 'assets/templates/' + $f + '.txt'
    if (-not (Test-Path $p)) { $p = 'assets/templates/' + $f }
    $b = [System.IO.File]::ReadAllBytes($p)
    $last = $b[$b.Length-1]
    Write-Host "$f last byte: 0x$($last.ToString('X2'))"
}