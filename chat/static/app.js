"use strict";

(function () {
    // WebSocket connection
    let socket = null;
    
    // Track our own nickname to style our messages differently
    let myNickname = null;
    
    // Track the last message we sent to detect our nickname
    let lastSentMessage = null;

    // DOM elements
    const messagesContainer = document.getElementById('messages');
    const chatContainer = document.getElementById('chat-container');
    const messageInput = document.getElementById('message-input');
    const sendButton = document.getElementById('send-button');
    const statusElement = document.getElementById('connection-status');

    // Maximum number of messages to keep in the chat
    const MAX_MESSAGES = 100;

    // Reconnection settings
    let reconnectAttempts = 0;
    const MAX_RECONNECT_ATTEMPTS = 10;
    const RECONNECT_DELAY_BASE = 1000; // 1 second

    // Initialize when DOM is ready
    document.addEventListener('DOMContentLoaded', function () {
        connect();
        setupEventListeners();
    });

    /**
     * Set up event listeners for user interaction
     */
    function setupEventListeners() {
        // Send message on Enter key
        messageInput.addEventListener('keydown', function (event) {
            if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault();
                sendMessage();
            }
        });

        // Send message on button click
        sendButton.addEventListener('click', sendMessage);
    }

    /**
     * Connect to the WebSocket server
     */
    function connect() {
        // Determine WebSocket URL based on current page protocol
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/chat`;

        setStatus('connecting', 'Connecting...');

        socket = new WebSocket(wsUrl);

        socket.onopen = handleOpen;
        socket.onclose = handleClose;
        socket.onerror = handleError;
        socket.onmessage = handleMessage;
    }

    /**
     * Handle WebSocket connection opened
     */
    function handleOpen() {
        console.log('WebSocket connected');
        setStatus('connected', 'Connected');
        enableInput(true);
        messageInput.focus();
        reconnectAttempts = 0;
    }

    /**
     * Handle WebSocket connection closed
     */
    function handleClose(event) {
        console.log('WebSocket closed:', event.code, event.reason);
        setStatus('disconnected', 'Disconnected');
        enableInput(false);

        // Attempt to reconnect
        if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
            reconnectAttempts++;
            const delay = RECONNECT_DELAY_BASE * Math.pow(1.5, reconnectAttempts - 1);
            console.log(`Reconnecting in ${delay}ms (attempt ${reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS})`);
            setTimeout(connect, delay);
        } else {
            addSystemMessage('Connection lost. Please refresh the page.');
        }
    }

    /**
     * Handle WebSocket error
     */
    function handleError(error) {
        console.error('WebSocket error:', error);
    }

    /**
     * Handle incoming WebSocket message
     */
    function handleMessage(event) {
        const data = event.data;
        
        // Check if it's a system message (starts with "* ")
        if (data.startsWith('* ')) {
            addSystemMessage(data.substring(2));
        } else {
            // Regular chat message
            const colonIndex = data.indexOf(': ');
            if (colonIndex !== -1) {
                const nickname = data.substring(0, colonIndex);
                const message = data.substring(colonIndex + 2);
                
                // Detect our own nickname from the first message we send
                if (!myNickname && lastSentMessage && message === lastSentMessage) {
                    myNickname = nickname;
                    console.log('Detected nickname:', myNickname);
                    lastSentMessage = null;
                }
                
                const isOwn = nickname === myNickname;
                addChatMessage(nickname, message, isOwn);
            } else {
                // Unknown format, show as system message
                addSystemMessage(data);
            }
        }
    }

    /**
     * Send a message to the server
     */
    function sendMessage() {
        const text = messageInput.value.trim();
        
        if (text && socket && socket.readyState === WebSocket.OPEN) {
            // Track this message to detect our nickname
            if (!myNickname) {
                lastSentMessage = text;
            }
            
            socket.send(text);
            messageInput.value = '';
            messageInput.focus();
        }
    }

    /**
     * Add a system message (join/leave notifications)
     */
    function addSystemMessage(text) {
        const div = document.createElement('div');
        div.className = 'message system';
        div.textContent = text;
        appendMessage(div);
    }

    /**
     * Add a chat message
     */
    function addChatMessage(nickname, message, isOwn) {
        const div = document.createElement('div');
        div.className = `message chat${isOwn ? ' own' : ''}`;
        
        const nickSpan = document.createElement('span');
        nickSpan.className = 'nickname';
        nickSpan.textContent = nickname + ': ';
        
        const textSpan = document.createElement('span');
        textSpan.className = 'text';
        textSpan.textContent = message;
        
        div.appendChild(nickSpan);
        div.appendChild(textSpan);
        
        appendMessage(div);
    }

    /**
     * Append a message element and manage scroll
     */
    function appendMessage(element) {
        messagesContainer.appendChild(element);
        
        // Auto-scroll to bottom
        chatContainer.scrollTop = chatContainer.scrollHeight;
        
        // Limit the number of messages
        while (messagesContainer.children.length > MAX_MESSAGES) {
            messagesContainer.removeChild(messagesContainer.firstChild);
        }
    }

    /**
     * Set the connection status display
     */
    function setStatus(state, text) {
        statusElement.className = `status ${state}`;
        statusElement.textContent = text;
    }

    /**
     * Enable or disable the input controls
     */
    function enableInput(enabled) {
        messageInput.disabled = !enabled;
        sendButton.disabled = !enabled;
    }
})();
