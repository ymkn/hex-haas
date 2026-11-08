# Shared helpers for the Haas setup scripts, dot-sourced by
# setup-haas-build-tools.ps1 and setup-haas-cpu-build-tools.ps1.

# Downloads flake often enough (GitHub release assets answering 504) to
# sink an unattended daily build, so every pinned download retries with
# backoff. The callers hash-verify the result as before.
function Get-PinnedDownload([string]$Uri, [string]$OutFile) {
    $ProgressPreference = 'SilentlyContinue'
    $delays = @(10, 20, 40, 80, 120)
    for ($attempt = 0; ; $attempt++) {
        try {
            Invoke-WebRequest $Uri -OutFile $OutFile
            return
        } catch {
            if ($attempt -ge $delays.Count) { throw }
            Write-Warning "Download of $Uri failed ($($_.Exception.Message)); retry $($attempt + 1) in $($delays[$attempt])s."
            Remove-Item $OutFile -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds $delays[$attempt]
        }
    }
}
