-- GhostLink Database Initialization Script
-- Creates the necessary tables and indexes for the GhostLink application

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Create enum types
CREATE TYPE session_type AS ENUM ('view', 'control', 'terminal', 'file_transfer');
CREATE TYPE session_status AS ENUM ('connecting', 'connected', 'disconnected', 'failed');
CREATE TYPE elevation_type AS ENUM ('run_as_admin', 'run_as_user', 'run_as_service', 'run_as_system', 'domain_admin', 'local_admin');
CREATE TYPE elevation_status AS ENUM ('pending', 'approved', 'denied', 'expired', 'active', 'completed', 'failed');
CREATE TYPE tool_category AS ENUM ('sysinternals', 'nirsoft', 'custom', 'scripts');
CREATE TYPE banner_type AS ENUM ('connection', 'security', 'maintenance', 'legal', 'custom');

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    azure_object_id VARCHAR(255) UNIQUE,
    tenant_id VARCHAR(255),
    roles TEXT[] DEFAULT '{}',
    is_active BOOLEAN DEFAULT true,
    last_login TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Agents (devices) table
CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    hostname VARCHAR(255) NOT NULL,
    platform VARCHAR(100) NOT NULL,
    architecture VARCHAR(50) NOT NULL,
    version VARCHAR(50) NOT NULL,
    public_key TEXT,
    owner_id UUID REFERENCES users(id) ON DELETE SET NULL,
    group_id UUID,
    tags TEXT[] DEFAULT '{}',
    is_online BOOLEAN DEFAULT false,
    last_seen TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Sessions table
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID REFERENCES agents(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    session_type session_type NOT NULL,
    status session_status DEFAULT 'connecting',
    ip_address INET,
    user_agent TEXT,
    started_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ended_at TIMESTAMP WITH TIME ZONE,
    duration_seconds INTEGER,
    bytes_transferred BIGINT DEFAULT 0
);

-- PAM elevation requests table
CREATE TABLE pam_elevation_requests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    session_id UUID REFERENCES sessions(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    requested_by VARCHAR(255) NOT NULL,
    user_domain VARCHAR(255),
    reason TEXT NOT NULL,
    target_process VARCHAR(255),
    target_command TEXT,
    elevation_type elevation_type NOT NULL,
    status elevation_status DEFAULT 'pending',
    requested_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,
    approved_by VARCHAR(255),
    approved_at TIMESTAMP WITH TIME ZONE,
    denied_reason TEXT,
    auto_approved BOOLEAN DEFAULT false,
    risk_score INTEGER DEFAULT 0
);

-- PAM audit log table
CREATE TABLE pam_audit_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    elevation_request_id UUID REFERENCES pam_elevation_requests(id) ON DELETE CASCADE,
    session_id UUID,
    user_id VARCHAR(255) NOT NULL,
    action VARCHAR(255) NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT,
    risk_score INTEGER DEFAULT 0,
    compliance_flags TEXT[] DEFAULT '{}',
    details JSONB
);

-- Terminal sessions table
CREATE TABLE terminal_sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    client_session_id UUID REFERENCES sessions(id) ON DELETE CASCADE,
    shell_type VARCHAR(50) NOT NULL,
    current_directory VARCHAR(500),
    is_elevated BOOLEAN DEFAULT false,
    started_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_activity TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ended_at TIMESTAMP WITH TIME ZONE,
    status VARCHAR(50) DEFAULT 'active'
);

-- Command history table
CREATE TABLE command_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    terminal_session_id UUID REFERENCES terminal_sessions(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    command TEXT NOT NULL,
    working_directory VARCHAR(500),
    executed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    exit_code INTEGER,
    duration_ms BIGINT,
    output_lines INTEGER DEFAULT 0,
    error_output BOOLEAN DEFAULT false
);

-- Toolbox tools table
CREATE TABLE toolbox_tools (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    category tool_category NOT NULL,
    version VARCHAR(50),
    description TEXT,
    executable_path VARCHAR(500) NOT NULL,
    arguments TEXT[] DEFAULT '{}',
    working_directory VARCHAR(500),
    requires_elevation BOOLEAN DEFAULT false,
    icon_path VARCHAR(500),
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Tool execution history table
CREATE TABLE tool_executions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tool_id UUID REFERENCES toolbox_tools(id) ON DELETE CASCADE,
    session_id UUID REFERENCES sessions(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    arguments TEXT[] DEFAULT '{}',
    executed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE,
    exit_code INTEGER,
    stdout TEXT,
    stderr TEXT,
    success BOOLEAN,
    error_message TEXT
);

-- Connection banners table
CREATE TABLE connection_banners (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    session_id UUID REFERENCES sessions(id) ON DELETE CASCADE,
    banner_type banner_type NOT NULL,
    title VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    company_name VARCHAR(255) NOT NULL,
    company_logo VARCHAR(500),
    security_classification VARCHAR(50),
    warning_text TEXT,
    compliance_info TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,
    acknowledgment_required BOOLEAN DEFAULT false,
    acknowledged_by TEXT[] DEFAULT '{}'
);

-- VPN peers table
CREATE TABLE vpn_peers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    peer_type VARCHAR(50) NOT NULL, -- 'tailscale' or 'wireguard'
    peer_id VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    public_key VARCHAR(255),
    endpoint VARCHAR(255),
    allowed_ips TEXT[] DEFAULT '{}',
    last_seen TIMESTAMP WITH TIME ZONE,
    is_online BOOLEAN DEFAULT false,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- OIDC sessions table
CREATE TABLE oidc_sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    session_token VARCHAR(255) UNIQUE NOT NULL,
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_used TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,
    ip_address INET,
    user_agent TEXT
);

-- Direct connect clients table
CREATE TABLE direct_connect_clients (
    id VARCHAR(255) PRIMARY KEY,
    password VARCHAR(255) NOT NULL,
    local_ip INET,
    external_ip INET,
    port INTEGER,
    nat_type VARCHAR(50),
    relay_server VARCHAR(255),
    last_seen TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Application settings table
CREATE TABLE app_settings (
    key VARCHAR(255) PRIMARY KEY,
    value JSONB NOT NULL,
    description TEXT,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_by UUID REFERENCES users(id)
);

-- Create indexes for better performance
CREATE INDEX idx_agents_owner_id ON agents(owner_id);
CREATE INDEX idx_agents_is_online ON agents(is_online);
CREATE INDEX idx_agents_last_seen ON agents(last_seen);
CREATE INDEX idx_sessions_agent_id ON sessions(agent_id);
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_status ON sessions(status);
CREATE INDEX idx_sessions_started_at ON sessions(started_at);
CREATE INDEX idx_pam_elevation_requests_session_id ON pam_elevation_requests(session_id);
CREATE INDEX idx_pam_elevation_requests_user_id ON pam_elevation_requests(user_id);
CREATE INDEX idx_pam_elevation_requests_status ON pam_elevation_requests(status);
CREATE INDEX idx_pam_audit_log_elevation_request_id ON pam_audit_log(elevation_request_id);
CREATE INDEX idx_pam_audit_log_timestamp ON pam_audit_log(timestamp);
CREATE INDEX idx_terminal_sessions_user_id ON terminal_sessions(user_id);
CREATE INDEX idx_terminal_sessions_client_session_id ON terminal_sessions(client_session_id);
CREATE INDEX idx_command_history_terminal_session_id ON command_history(terminal_session_id);
CREATE INDEX idx_command_history_executed_at ON command_history(executed_at);
CREATE INDEX idx_tool_executions_tool_id ON tool_executions(tool_id);
CREATE INDEX idx_tool_executions_session_id ON tool_executions(session_id);
CREATE INDEX idx_tool_executions_executed_at ON tool_executions(executed_at);
CREATE INDEX idx_connection_banners_session_id ON connection_banners(session_id);
CREATE INDEX idx_oidc_sessions_user_id ON oidc_sessions(user_id);
CREATE INDEX idx_oidc_sessions_session_token ON oidc_sessions(session_token);
CREATE INDEX idx_oidc_sessions_expires_at ON oidc_sessions(expires_at);

-- Create trigger function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Add update triggers
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_agents_updated_at BEFORE UPDATE ON agents FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_toolbox_tools_updated_at BEFORE UPDATE ON toolbox_tools FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_vpn_peers_updated_at BEFORE UPDATE ON vpn_peers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default application settings
INSERT INTO app_settings (key, value, description) VALUES 
('branding', '{
    "company_name": "GhostLink Remote Access",
    "primary_color": "#0d6efd",
    "secondary_color": "#6c757d",
    "accent_color": "#198754",
    "company_logo": "/assets/logo.png"
}', 'Default branding configuration'),

('pam_config', '{
    "require_justification": true,
    "approval_required_for_admin": true,
    "approval_required_for_system": true,
    "max_elevation_duration_hours": 2,
    "audit_retention_days": 90
}', 'PAM system configuration'),

('terminal_config', '{
    "default_shell": "bash",
    "max_sessions_per_user": 5,
    "session_timeout_minutes": 60,
    "max_output_buffer_lines": 10000
}', 'Terminal system configuration'),

('vpn_config', '{
    "tailscale_enabled": false,
    "wireguard_enabled": false,
    "require_vpn_for_admin": false
}', 'VPN integration configuration');

-- Create a default admin user (will be updated by OIDC on first login)
INSERT INTO users (id, email, display_name, roles, is_active) VALUES 
(uuid_generate_v4(), 'admin@ghostlink.local', 'Default Administrator', ARRAY['admin', 'user'], true);

COMMIT;