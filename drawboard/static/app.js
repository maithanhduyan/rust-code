"use strict";

(function () {
    // Configuration
    const CANVAS_WIDTH = 900;
    const CANVAS_HEIGHT = 600;
    const PING_INTERVAL = 30000; // 30 seconds

    // State
    let socket = null;
    let pingTimerId = null;
    let isStarted = false;
    let playerCount = 0;
    let nextMsgId = 1;

    // Pending paths (not yet confirmed by server)
    let pathsNotHandled = [];

    // Drawing state
    let isDrawing = false;
    let lastX = 0;
    let lastY = 0;
    let startX = 0;
    let startY = 0;

    // Current tool settings
    let currentTool = 1; // 1=Brush, 2=Line, 3=Rectangle, 4=Ellipse
    let currentThickness = 6;
    let currentColor = [0, 0, 0, 255]; // RGBA

    // Canvas layers
    let canvasDisplay = null;
    let canvasBackground = null;
    let canvasServerImage = null;

    let ctxDisplay = null;
    let ctxBackground = null;
    let ctxServerImage = null;

    // DOM Elements
    let drawContainer = null;
    let playerCountLabel = null;
    let connectionStatus = null;

    // Console logging
    const Console = {
        log: function (message, type = "") {
            const consoleEl = document.getElementById("console");
            const p = document.createElement("p");
            const time = new Date().toLocaleTimeString();
            p.textContent = `[${time}] ${message}`;
            if (type) p.className = type;
            consoleEl.appendChild(p);
            consoleEl.scrollTop = consoleEl.scrollHeight;
            console.log(`[${type || "log"}] ${message}`);
        },
    };

    // Initialize when DOM is ready
    document.addEventListener("DOMContentLoaded", init);

    function init() {
        Console.log("Initializing Drawboard...", "info");

        drawContainer = document.getElementById("drawContainer");
        playerCountLabel = document.getElementById("playerCount");
        connectionStatus = document.getElementById("connectionStatus");

        // Setup canvases
        setupCanvases();

        // Setup toolbar
        setupToolbar();

        // Connect to WebSocket
        connect();
    }

    function setupCanvases() {
        // Get the visible canvas
        canvasDisplay = document.getElementById("canvas");
        canvasDisplay.width = CANVAS_WIDTH;
        canvasDisplay.height = CANVAS_HEIGHT;
        ctxDisplay = canvasDisplay.getContext("2d");

        // Create background canvas (server state + pending drawings)
        canvasBackground = document.createElement("canvas");
        canvasBackground.width = CANVAS_WIDTH;
        canvasBackground.height = CANVAS_HEIGHT;
        ctxBackground = canvasBackground.getContext("2d");

        // Create server image canvas (confirmed server state)
        canvasServerImage = document.createElement("canvas");
        canvasServerImage.width = CANVAS_WIDTH;
        canvasServerImage.height = CANVAS_HEIGHT;
        ctxServerImage = canvasServerImage.getContext("2d");

        // Fill with white
        ctxServerImage.fillStyle = "#ffffff";
        ctxServerImage.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

        // Setup event listeners
        canvasDisplay.addEventListener("mousedown", handleMouseDown);
        canvasDisplay.addEventListener("mousemove", handleMouseMove);
        canvasDisplay.addEventListener("mouseup", handleMouseUp);
        canvasDisplay.addEventListener("mouseleave", handleMouseUp);

        // Touch events
        canvasDisplay.addEventListener("touchstart", handleTouchStart, { passive: false });
        canvasDisplay.addEventListener("touchmove", handleTouchMove, { passive: false });
        canvasDisplay.addEventListener("touchend", handleTouchEnd, { passive: false });
    }

    function setupToolbar() {
        document.getElementById("toolSelect").addEventListener("change", (e) => {
            currentTool = parseInt(e.target.value);
            Console.log(`Tool changed to: ${getToolName(currentTool)}`);
        });

        document.getElementById("thicknessSelect").addEventListener("change", (e) => {
            currentThickness = parseInt(e.target.value);
        });

        document.getElementById("colorPicker").addEventListener("change", (e) => {
            const hex = e.target.value;
            currentColor[0] = parseInt(hex.substr(1, 2), 16);
            currentColor[1] = parseInt(hex.substr(3, 2), 16);
            currentColor[2] = parseInt(hex.substr(5, 2), 16);
        });

        const alphaSlider = document.getElementById("alphaSlider");
        const alphaValue = document.getElementById("alphaValue");
        alphaSlider.addEventListener("input", (e) => {
            currentColor[3] = parseInt(e.target.value);
            alphaValue.textContent = Math.round((currentColor[3] / 255) * 100) + "%";
        });
    }

    function getToolName(tool) {
        const names = { 1: "Brush", 2: "Line", 3: "Rectangle", 4: "Ellipse" };
        return names[tool] || "Unknown";
    }

    function connect() {
        const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
        const host = window.location.host;
        const url = `${protocol}//${host}/ws/drawboard`;

        Console.log(`Connecting to ${url}...`);

        socket = new WebSocket(url);

        socket.onopen = function () {
            Console.log("WebSocket connection opened.", "info");
            updateConnectionStatus(true);

            // Start ping timer
            pingTimerId = setInterval(() => {
                if (socket.readyState === WebSocket.OPEN) {
                    socket.send("0"); // Pong message
                }
            }, PING_INTERVAL);
        };

        socket.onclose = function (event) {
            Console.log(
                `WebSocket closed: ${event.reason || "Connection closed"}`,
                "error"
            );
            clearInterval(pingTimerId);
            isStarted = false;
            updateConnectionStatus(false);
        };

        socket.onerror = function (error) {
            Console.log("WebSocket error occurred.", "error");
        };

        socket.onmessage = handleMessage;
    }

    function updateConnectionStatus(connected) {
        if (connected) {
            connectionStatus.textContent = "Connected";
            connectionStatus.className = "status connected";
        } else {
            connectionStatus.textContent = "Disconnected";
            connectionStatus.className = "status disconnected";
            drawContainer.classList.add("hidden");
        }
    }

    function handleMessage(event) {
        // Check if binary message (PNG image)
        if (event.data instanceof Blob) {
            handleBinaryMessage(event.data);
            return;
        }

        const messages = event.data.split(";");

        for (const msg of messages) {
            if (msg.length === 0) continue;

            const type = msg.charAt(0);
            const content = msg.substring(1);

            switch (type) {
                case "0": // Error
                    Console.log(`Error: ${content}`, "error");
                    alert(content);
                    break;

                case "1": // Draw message(s)
                    handleDrawMessages(content);
                    break;

                case "2": // Image message (player count)
                    playerCount = parseInt(content);
                    updatePlayerCount();
                    Console.log(`Joined room with ${playerCount} player(s).`, "info");
                    // Next message will be binary PNG
                    break;

                case "3": // Player changed
                    if (content === "+") {
                        playerCount++;
                        Console.log("A player joined.");
                    } else {
                        playerCount--;
                        Console.log("A player left.");
                    }
                    updatePlayerCount();
                    break;
            }
        }
    }

    function handleBinaryMessage(blob) {
        const url = URL.createObjectURL(blob);
        const img = new Image();

        img.onload = function () {
            // Draw server image
            ctxServerImage.drawImage(img, 0, 0);

            // Initialize display
            updateDisplay();

            // Show drawing container
            drawContainer.classList.remove("hidden");
            isStarted = true;

            URL.revokeObjectURL(url);
            Console.log("Room image loaded.", "info");
        };

        img.onerror = function () {
            Console.log("Failed to load room image.", "error");
            URL.revokeObjectURL(url);
        };

        img.src = url;
    }

    function handleDrawMessages(content) {
        // Format: "lastMsgId,drawData" or "msg1|msg2|..."
        const messages = content.split("|");
        let maxLastHandledId = 0;

        for (const msgStr of messages) {
            const parts = msgStr.split(",");
            const lastHandledId = parseInt(parts[0]);
            maxLastHandledId = Math.max(maxLastHandledId, lastHandledId);

            // Parse draw message
            const drawMsg = {
                type: parseInt(parts[1]),
                colorR: parseInt(parts[2]),
                colorG: parseInt(parts[3]),
                colorB: parseInt(parts[4]),
                colorA: parseInt(parts[5]),
                thickness: parseFloat(parts[6]),
                x1: parseFloat(parts[7]),
                y1: parseFloat(parts[8]),
                x2: parseFloat(parts[9]),
                y2: parseFloat(parts[10]),
            };

            // Draw on server canvas
            drawPath(ctxServerImage, drawMsg);
        }

        // Remove handled paths from pending queue
        while (
            pathsNotHandled.length > 0 &&
            pathsNotHandled[0].id <= maxLastHandledId
        ) {
            pathsNotHandled.shift();
        }

        // Update display
        updateDisplay();
    }

    function drawPath(ctx, msg) {
        ctx.save();
        ctx.lineCap = "round";
        ctx.lineJoin = "miter";
        ctx.lineWidth = msg.thickness;
        ctx.strokeStyle = `rgba(${msg.colorR}, ${msg.colorG}, ${msg.colorB}, ${msg.colorA / 255})`;

        ctx.beginPath();

        if (msg.x1 === msg.x2 && msg.y1 === msg.y2) {
            // Draw a point
            ctx.arc(msg.x1, msg.y1, 0.5, 0, Math.PI * 2);
        } else if (msg.type === 1 || msg.type === 2) {
            // Brush or Line
            ctx.moveTo(msg.x1, msg.y1);
            ctx.lineTo(msg.x2, msg.y2);
        } else if (msg.type === 3) {
            // Rectangle
            const x = Math.min(msg.x1, msg.x2);
            const y = Math.min(msg.y1, msg.y2);
            const w = Math.abs(msg.x2 - msg.x1);
            const h = Math.abs(msg.y2 - msg.y1);
            ctx.rect(x, y, w, h);
        } else if (msg.type === 4) {
            // Ellipse
            const cx = (msg.x1 + msg.x2) / 2;
            const cy = (msg.y1 + msg.y2) / 2;
            const rx = Math.abs(msg.x2 - msg.x1) / 2;
            const ry = Math.abs(msg.y2 - msg.y1) / 2;
            ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
        }

        ctx.stroke();
        ctx.restore();
    }

    function updateDisplay() {
        // Copy server image to background
        ctxBackground.drawImage(canvasServerImage, 0, 0);

        // Draw pending paths on background
        for (const container of pathsNotHandled) {
            drawPath(ctxBackground, container.path);
        }

        // Copy background to display
        ctxDisplay.drawImage(canvasBackground, 0, 0);
    }

    function updatePlayerCount() {
        playerCountLabel.textContent = `Players: ${playerCount}`;
    }

    function getCanvasCoords(event) {
        const rect = canvasDisplay.getBoundingClientRect();
        const scaleX = canvasDisplay.width / rect.width;
        const scaleY = canvasDisplay.height / rect.height;
        return {
            x: (event.clientX - rect.left) * scaleX,
            y: (event.clientY - rect.top) * scaleY,
        };
    }

    function getTouchCoords(event) {
        const touch = event.touches[0] || event.changedTouches[0];
        return getCanvasCoords(touch);
    }

    // Mouse event handlers
    function handleMouseDown(event) {
        if (!isStarted) return;

        const coords = getCanvasCoords(event);
        startDrawing(coords.x, coords.y);
    }

    function handleMouseMove(event) {
        if (!isStarted || !isDrawing) return;

        const coords = getCanvasCoords(event);
        continueDrawing(coords.x, coords.y);
    }

    function handleMouseUp(event) {
        if (!isStarted || !isDrawing) return;

        const coords = getCanvasCoords(event);
        endDrawing(coords.x, coords.y);
    }

    // Touch event handlers
    function handleTouchStart(event) {
        event.preventDefault();
        if (!isStarted) return;

        const coords = getTouchCoords(event);
        startDrawing(coords.x, coords.y);
    }

    function handleTouchMove(event) {
        event.preventDefault();
        if (!isStarted || !isDrawing) return;

        const coords = getTouchCoords(event);
        continueDrawing(coords.x, coords.y);
    }

    function handleTouchEnd(event) {
        event.preventDefault();
        if (!isStarted || !isDrawing) return;

        const coords = getTouchCoords(event);
        endDrawing(coords.x, coords.y);
    }

    function startDrawing(x, y) {
        isDrawing = true;
        startX = x;
        startY = y;
        lastX = x;
        lastY = y;

        if (currentTool === 1) {
            // Brush: start with a point
            sendDrawMessage(x, y, x, y);
        }
    }

    function continueDrawing(x, y) {
        if (currentTool === 1) {
            // Brush: continuous drawing
            sendDrawMessage(lastX, lastY, x, y);
            lastX = x;
            lastY = y;
        } else {
            // Preview shape on display canvas
            updateDisplay();
            drawPreview(x, y);
        }
    }

    function endDrawing(x, y) {
        if (!isDrawing) return;
        isDrawing = false;

        if (currentTool !== 1) {
            // Line, Rectangle, Ellipse: send final shape
            sendDrawMessage(startX, startY, x, y);
        }

        updateDisplay();
    }

    function drawPreview(x, y) {
        const msg = {
            type: currentTool,
            colorR: currentColor[0],
            colorG: currentColor[1],
            colorB: currentColor[2],
            colorA: currentColor[3],
            thickness: currentThickness,
            x1: startX,
            y1: startY,
            x2: x,
            y2: y,
        };

        drawPath(ctxDisplay, msg);
    }

    function sendDrawMessage(x1, y1, x2, y2) {
        const msg = {
            type: currentTool,
            colorR: currentColor[0],
            colorG: currentColor[1],
            colorB: currentColor[2],
            colorA: currentColor[3],
            thickness: currentThickness,
            x1: x1,
            y1: y1,
            x2: x2,
            y2: y2,
        };

        const msgId = nextMsgId++;

        // Add to pending queue
        pathsNotHandled.push({ id: msgId, path: msg });

        // Build message string: "1<msgId>|type,R,G,B,A,thickness,x1,y1,x2,y2"
        const msgStr =
            `1${msgId}|${msg.type},${msg.colorR},${msg.colorG},${msg.colorB},` +
            `${msg.colorA},${msg.thickness},${msg.x1},${msg.y1},${msg.x2},${msg.y2}`;

        // Send to server
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(msgStr);
        }

        // Draw on background immediately (optimistic update)
        drawPath(ctxBackground, msg);
        updateDisplay();
    }
})();
