// AtlasConnect Dashboard JavaScript
class AtlasConnectDashboard {
    constructor() {
        this.apiBase = '/api/v1';
        this.wsUrl = `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/api/v1/relay`;
        this.ws = null;
        this.reconnectInterval = 5000;
        this.maxReconnectAttempts = 10;
        this.reconnectAttempts = 0;
        
        this.init();
    }

    async init() {
        await this.loadDashboardData();
        this.connectWebSocket();
        this.setupEventListeners();
        
        // Refresh data every 30 seconds
        setInterval(() => this.loadDashboardData(), 30000);
    }

    async loadDashboardData() {
        try {
            const [status, agents, sessions] = await Promise.all([
                this.fetchServerStatus(),
                this.fetchAgents(),
                this.fetchSessions()
            ]);

            this.updateStats(status);
            this.updateAgentsTable(agents);
            this.updateSessionsTable(sessions);
        } catch (error) {
            console.error('Error loading dashboard data:', error);
            this.showNotification('Error loading dashboard data', 'error');
        }
    }

    async fetchServerStatus() {
        const response = await fetch(`${this.apiBase}/status`);
        if (!response.ok) throw new Error('Failed to fetch server status');
        return response.json();
    }

    async fetchAgents() {
        const response = await fetch(`${this.apiBase}/agents`);
        if (!response.ok) throw new Error('Failed to fetch agents');
        return response.json();
    }

    async fetchSessions() {
        const response = await fetch(`${this.apiBase}/sessions`);
        if (!response.ok) throw new Error('Failed to fetch sessions');
        return response.json();
    }

    updateStats(status) {
        document.getElementById('active-agents').textContent = status.active_agents || 0;
        document.getElementById('online-users').textContent = status.total_connections || 0;
        document.getElementById('active-sessions').textContent = status.active_sessions || 0;
    }

    updateAgentsTable(agents) {
        const tableBody = document.getElementById('agents-table');
        
        if (!agents || agents.length === 0) {
            tableBody.innerHTML = `
                <tr>
                    <td colspan="5" class="px-6 py-4 text-center text-gray-500">
                        No agents connected
                    </td>
                </tr>
            `;
            return;
        }

        tableBody.innerHTML = agents.slice(0, 5).map(agent => `
            <tr class="hover:bg-gray-50">
                <td class="px-6 py-4 whitespace-nowrap">
                    <div class="flex items-center">
                        <div class="flex-shrink-0 h-10 w-10">
                            <div class="h-10 w-10 rounded-full bg-blue-100 flex items-center justify-center">
                                <i class="fas fa-desktop text-blue-600"></i>
                            </div>
                        </div>
                        <div class="ml-4">
                            <div class="text-sm font-medium text-gray-900">${agent.name}</div>
                            <div class="text-sm text-gray-500">${agent.hostname}</div>
                        </div>
                    </div>
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                    <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                        <span class="w-2 h-2 bg-green-400 rounded-full mr-1"></span>
                        Online
                    </span>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                    ${agent.platform} ${agent.architecture}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                    Just now
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium">
                    <button onclick="dashboard.startSession('${agent.id}')" 
                            class="text-blue-600 hover:text-blue-900 mr-3">
                        <i class="fas fa-video mr-1"></i>Connect
                    </button>
                    <button class="text-gray-600 hover:text-gray-900">
                        <i class="fas fa-cog mr-1"></i>Settings
                    </button>
                </td>
            </tr>
        `).join('');
    }

    updateSessionsTable(sessions) {
        const tableBody = document.getElementById('sessions-table');
        
        if (!sessions || sessions.length === 0) {
            tableBody.innerHTML = `
                <tr>
                    <td colspan="5" class="px-6 py-4 text-center text-gray-500">
                        No recent sessions
                    </td>
                </tr>
            `;
            return;
        }

        tableBody.innerHTML = sessions.slice(0, 5).map(session => `
            <tr class="hover:bg-gray-50">
                <td class="px-6 py-4 whitespace-nowrap">
                    <div class="text-sm font-medium text-gray-900">${session.id.substring(0, 8)}</div>
                    <div class="text-sm text-gray-500">${session.session_type}</div>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                    Agent ${session.agent_id.substring(0, 8)}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                    User ${session.user_id.substring(0, 8)}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                    ${session.duration_seconds ? this.formatDuration(session.duration_seconds) : 'In progress'}
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                    <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${this.getStatusClass(session.status)}">
                        ${session.status}
                    </span>
                </td>
            </tr>
        `).join('');
    }

    getStatusClass(status) {
        switch (status) {
            case 'active': return 'bg-green-100 text-green-800';
            case 'ended': return 'bg-gray-100 text-gray-800';
            case 'failed': return 'bg-red-100 text-red-800';
            case 'pending': return 'bg-yellow-100 text-yellow-800';
            default: return 'bg-gray-100 text-gray-800';
        }
    }

    formatDuration(seconds) {
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = seconds % 60;
        
        if (hours > 0) {
            return `${hours}h ${minutes}m`;
        } else if (minutes > 0) {
            return `${minutes}m ${secs}s`;
        } else {
            return `${secs}s`;
        }
    }

    async startSession(agentId) {
        try {
            const response = await fetch(`${this.apiBase}/sessions`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    agent_id: agentId,
                    session_type: 'screen'
                })
            });

            if (!response.ok) throw new Error('Failed to create session');
            
            const session = await response.json();
            
            // Open session in new window/tab
            window.open(`/client/session/${session.session_id}`, '_blank');
            
            this.showNotification('Session started successfully', 'success');
            
            // Refresh data to show new session
            setTimeout(() => this.loadDashboardData(), 1000);
            
        } catch (error) {
            console.error('Error starting session:', error);
            this.showNotification('Failed to start session', 'error');
        }
    }

    connectWebSocket() {
        try {
            this.ws = new WebSocket(this.wsUrl);
            
            this.ws.onopen = () => {
                console.log('WebSocket connected');
                this.reconnectAttempts = 0;
                
                // Send authentication if needed
                this.ws.send(JSON.stringify({
                    type: 'Authenticate',
                    token: 'dashboard-token' // TODO: Use real JWT token
                }));
            };

            this.ws.onmessage = (event) => {
                try {
                    const message = JSON.parse(event.data);
                    this.handleWebSocketMessage(message);
                } catch (error) {
                    console.error('Error parsing WebSocket message:', error);
                }
            };

            this.ws.onclose = () => {
                console.log('WebSocket disconnected');
                this.attemptReconnect();
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
            };

        } catch (error) {
            console.error('Error connecting WebSocket:', error);
            this.attemptReconnect();
        }
    }

    handleWebSocketMessage(message) {
        switch (message.type) {
            case 'AuthResult':
                if (message.success) {
                    console.log('WebSocket authenticated');
                } else {
                    console.error('WebSocket authentication failed:', message.message);
                }
                break;
                
            case 'AgentConnected':
            case 'AgentDisconnected':
            case 'SessionStarted':
            case 'SessionEnded':
                // Refresh dashboard data when events occur
                this.loadDashboardData();
                break;
                
            default:
                console.log('Unhandled WebSocket message:', message);
        }
    }

    attemptReconnect() {
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.reconnectAttempts++;
            console.log(`Attempting to reconnect WebSocket (${this.reconnectAttempts}/${this.maxReconnectAttempts})...`);
            
            setTimeout(() => {
                this.connectWebSocket();
            }, this.reconnectInterval);
        } else {
            console.error('Max WebSocket reconnect attempts reached');
            this.showNotification('Connection lost. Please refresh the page.', 'error');
        }
    }

    setupEventListeners() {
        // Add event listeners for buttons, etc.
        // This would be expanded based on actual UI interactions needed
    }

    showNotification(message, type = 'info') {
        // Simple notification system - could be replaced with a proper toast library
        const notification = document.createElement('div');
        notification.className = `fixed top-4 right-4 p-4 rounded-lg shadow-lg text-white z-50 ${
            type === 'success' ? 'bg-green-500' : 
            type === 'error' ? 'bg-red-500' : 
            'bg-blue-500'
        }`;
        notification.textContent = message;
        
        document.body.appendChild(notification);
        
        setTimeout(() => {
            notification.remove();
        }, 5000);
    }
}

// Initialize dashboard when page loads
let dashboard;
document.addEventListener('DOMContentLoaded', () => {
    dashboard = new AtlasConnectDashboard();
});
