param(
    [string]$BinaryPath = ".\target\debug\Chataigne2.exe",
    [string]$BindAddress = "127.0.0.1:17010",
    [string]$FrontendOrigin = "http://127.0.0.1:5173"
)

$ErrorActionPreference = "Stop"
$previousBind = $env:GC_UI_BIND
$previousFrontendUrl = $env:GC_UI_FRONTEND_URL
$process = $null

try {
    $env:GC_UI_BIND = $BindAddress
    $env:GC_UI_FRONTEND_URL = $FrontendOrigin
    $process = Start-Process `
        -FilePath $BinaryPath `
        -ArgumentList @("--headless", "--no-frontend") `
        -PassThru `
        -WindowStyle Hidden

    $hostName, $portText = $BindAddress -split ":", 2
    $port = [int]$portText
    $ready = $false
    for ($attempt = 0; $attempt -lt 80; $attempt++) {
        try {
            $probe = [System.Net.Sockets.TcpClient]::new()
            $probe.Connect($hostName, $port)
            $probe.Dispose()
            $ready = $true
            break
        }
        catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if (-not $ready) {
        throw "Runtime did not begin listening on $BindAddress"
    }

    Add-Type -AssemblyName System.Net.Http
    $baseUrl = "http://localhost:$port/api/ui"
    $http = [System.Net.Http.HttpClient]::new()

    $preflightRequest = [System.Net.Http.HttpRequestMessage]::new(
        [System.Net.Http.HttpMethod]::Options,
        "$baseUrl/snapshot"
    )
    $null = $preflightRequest.Headers.TryAddWithoutValidation("Origin", $FrontendOrigin)
    $null = $preflightRequest.Headers.TryAddWithoutValidation("Access-Control-Request-Method", "POST")
    $preflight = $http.SendAsync($preflightRequest).GetAwaiter().GetResult()
    $allowOriginValues = [System.Collections.Generic.IEnumerable[string]]$null
    $hasAllowOrigin = $preflight.Headers.TryGetValues(
        "Access-Control-Allow-Origin",
        [ref]$allowOriginValues
    )
    $allowOrigin = if ($hasAllowOrigin) {
        $allowOriginValues | Select-Object -First 1
    }
    else {
        ""
    }
    if ([int]$preflight.StatusCode -ne 204 -or $allowOrigin -ne $FrontendOrigin) {
        throw "CORS preflight failed: status=$([int]$preflight.StatusCode) allow-origin=$allowOrigin"
    }

    foreach ($endpoint in @("snapshot", "metrics", "connection-info")) {
        $method = if ($endpoint -eq "snapshot") {
            [System.Net.Http.HttpMethod]::Post
        }
        else {
            [System.Net.Http.HttpMethod]::Get
        }
        $request = [System.Net.Http.HttpRequestMessage]::new(
            $method,
            "$baseUrl/$endpoint"
        )
        if ($endpoint -eq "snapshot") {
            $request.Content = [System.Net.Http.StringContent]::new(
                "{}",
                [System.Text.Encoding]::UTF8,
                "application/json"
            )
        }
        $null = $request.Headers.TryAddWithoutValidation("Origin", $FrontendOrigin)
        $response = $http.SendAsync($request).GetAwaiter().GetResult()
        if (-not $response.IsSuccessStatusCode) {
            throw "GET /api/ui/$endpoint failed: status=$([int]$response.StatusCode)"
        }
        $request.Dispose()
        $response.Dispose()
    }

    $socket = [System.Net.WebSockets.ClientWebSocket]::new()
    $socket.Options.SetRequestHeader("Origin", $FrontendOrigin)
    $connectTask = $socket.ConnectAsync(
        [Uri]"ws://localhost:$port/api/ui/ws",
        [Threading.CancellationToken]::None
    )
    $null = $connectTask.GetAwaiter().GetResult()
    if ($socket.State -ne [System.Net.WebSockets.WebSocketState]::Open) {
        throw "WebSocket handshake failed: state=$($socket.State)"
    }

    $helloBytes = [System.Text.Encoding]::UTF8.GetBytes(
        '{"kind":"hello","protocol_version":"0.1.0","client_instance_id":"runtime-connectivity-check"}'
    )
    $helloSegment = [System.ArraySegment[byte]]::new($helloBytes)
    $sendTask = $socket.SendAsync(
        $helloSegment,
        [System.Net.WebSockets.WebSocketMessageType]::Text,
        $true,
        [Threading.CancellationToken]::None
    )
    $null = $sendTask.GetAwaiter().GetResult()

    $receiveBuffer = [byte[]]::new(4096)
    $receiveSegment = [System.ArraySegment[byte]]::new($receiveBuffer)
    $receiveTimeout = [Threading.CancellationTokenSource]::new()
    $receiveTimeout.CancelAfter(5000)
    $receiveTask = $socket.ReceiveAsync($receiveSegment, $receiveTimeout.Token)
    $received = $receiveTask.GetAwaiter().GetResult()
    if ($received.MessageType -ne [System.Net.WebSockets.WebSocketMessageType]::Text) {
        throw "Expected WebSocket hello text frame, got $($received.MessageType)"
    }
    $serverHello = [System.Text.Encoding]::UTF8.GetString(
        $receiveBuffer,
        0,
        $received.Count
    )
    if ($serverHello -notmatch '"kind"\s*:\s*"hello"') {
        throw "Expected server hello frame, got: $serverHello"
    }

    Write-Output (
        "Runtime connectivity passed: preflight=204, snapshot=200, metrics=200, " +
        "connection-info=200, websocket=Open+Hello"
    )

    $socket.Dispose()
    $preflightRequest.Dispose()
    $preflight.Dispose()
    $http.Dispose()
}
finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
    }
    $env:GC_UI_BIND = $previousBind
    $env:GC_UI_FRONTEND_URL = $previousFrontendUrl
}
