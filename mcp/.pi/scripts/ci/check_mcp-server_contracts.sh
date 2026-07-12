#!/usr/bin/env bash
# Check MCP Server Contract Implementation
#
# Validates that all interfaces defined in the contract freeze have
# concrete implementations. Uses grep/find to detect implementation
# classes for each interface.
#
# Usage: bash .pi/scripts/ci/check_mcp-server_contracts.sh
# Exit: 0 = all interfaces implemented, 1 = violations found

set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || echo ".")"

PASS=0
FAIL=0

check_impl() {
    local interface="$1"
    local pattern="$2"
    local description="$3"
    
    if grep -q "$pattern" mcp/src/ --include="*.rs" -r 2>/dev/null; then
        echo "  ✓ PASS: $description"
        PASS=$((PASS + 1))
    else
        echo "  ✗ FAIL: $description — no implementation found for $interface"
        FAIL=$((FAIL + 1))
    fi
}

check_impl_tests() {
    local interface="$1"
    local pattern="$2"
    local description="$3"
    
    if grep -q "$pattern" mcp/tests/ --include="*.rs" -r 2>/dev/null; then
        echo "  ✓ PASS: $description"
        PASS=$((PASS + 1))
    else
        echo "  ✗ FAIL: $description — no implementation found for $interface"
        FAIL=$((FAIL + 1))
    fi
}

echo "═══ MCP Server Contract Implementation Checks ═══"
echo ""

# Domain entities
check_impl "McpServer" "pub struct McpServer" "McpServer aggregate defined"
check_impl "McpServer::new" "fn new" "McpServer::new() constructor"
check_impl "McpServer::start" "fn start" "McpServer::start() lifecycle"
check_impl "McpServer::shutdown" "fn shutdown" "McpServer::shutdown() lifecycle"
check_impl "McpServer::create_session" "fn create_session" "McpServer::create_session() session management"

check_impl "ToolRegistry" "pub struct ToolRegistry" "ToolRegistry aggregate defined"
check_impl "ToolRegistry::register" "fn register" "ToolRegistry::register() tool registration"
check_impl "ToolRegistry::unregister" "fn unregister" "ToolRegistry::unregister() tool removal"
check_impl "ToolRegistry::list_schemas" "fn list_schemas" "ToolRegistry::list_schemas() listing"

# Value objects
check_impl "JsonRpcMessage" "pub struct JsonRpcMessage" "JsonRpcMessage value object"
check_impl "ToolSchema" "pub struct ToolSchema" "ToolSchema value object"
check_impl "ResourceSchema" "pub struct ResourceSchema" "ResourceSchema value object"
check_impl "PromptSchema" "pub struct PromptSchema" "PromptSchema value object"
check_impl "ServerCapabilities" "pub struct ServerCapabilities" "ServerCapabilities value object"
check_impl "SessionId" "pub struct SessionId" "SessionId value object"
check_impl "ToolHandler" "pub trait ToolHandler" "ToolHandler trait"

# Error types
check_impl "McpServerError" "pub enum McpServerError" "McpServerError enum"
check_impl "SessionError" "pub enum SessionError" "SessionError enum"
check_impl "RegistrationError" "pub enum RegistrationError" "RegistrationError enum"

# Domain events
check_impl "McpServerEvent" "pub enum McpServerEvent" "McpServerEvent enum"
check_impl "McpSessionStarted" "McpSessionStarted" "McpSessionStarted event variant"
check_impl "McpSessionEnded" "McpSessionEnded" "McpSessionEnded event variant"
check_impl "ToolCallReceived" "ToolCallReceived" "ToolCallReceived event variant"
check_impl "ToolCallCompleted" "ToolCallCompleted" "ToolCallCompleted event variant"
check_impl "ToolCallFailed" "ToolCallFailed" "ToolCallFailed event variant"
check_impl "ToolRegistered" "ToolRegistered" "ToolRegistered event variant"

# Service traits
check_impl "McpServerService" "pub trait McpServerService" "McpServerService trait"
check_impl "ToolRegistryService" "pub trait ToolRegistryService" "ToolRegistryService trait"
check_impl "SessionService" "pub trait SessionService" "SessionService trait"

# Implementation services
check_impl "McpServerServiceImpl" "pub struct McpServerServiceImpl" "McpServerServiceImpl impl"
check_impl "ToolRegistryServiceImpl" "pub struct ToolRegistryServiceImpl" "ToolRegistryServiceImpl impl"
check_impl "SessionServiceImpl" "pub struct SessionServiceImpl" "SessionServiceImpl impl"

# Repository interfaces
check_impl "McpServerRepository" "pub trait McpServerRepository" "McpServerRepository trait"
check_impl "ToolRegistryRepository" "pub trait ToolRegistryRepository" "ToolRegistryRepository trait"
check_impl "SessionRepository" "pub trait SessionRepository" "SessionRepository trait"

# In-memory repositories
check_impl "InMemoryMcpServerRepository" "pub struct InMemoryMcpServerRepository" "InMemoryMcpServerRepository"
check_impl "InMemoryToolRegistryRepository" "pub struct InMemoryToolRegistryRepository" "InMemoryToolRegistryRepository"
check_impl "InMemorySessionRepository" "pub struct InMemorySessionRepository" "InMemorySessionRepository"

# MCP handlers
check_impl "InitializeHandler" "pub trait InitializeHandler" "InitializeHandler trait"
check_impl "ListToolsHandler" "pub trait ListToolsHandler" "ListToolsHandler trait"
check_impl "CallToolHandler" "pub trait CallToolHandler" "CallToolHandler trait"
check_impl "ListResourcesHandler" "pub trait ListResourcesHandler" "ListResourcesHandler trait"
check_impl "ReadResourceHandler" "pub trait ReadResourceHandler" "ReadResourceHandler trait"
check_impl "ListPromptsHandler" "pub trait ListPromptsHandler" "ListPromptsHandler trait"
check_impl "GetPromptHandler" "pub trait GetPromptHandler" "GetPromptHandler trait"
check_impl "CancelledHandler" "pub trait CancelledHandler" "CancelledHandler trait"

# Concrete handler implementations
check_impl "InitializeHandlerImpl" "pub struct InitializeHandlerImpl" "InitializeHandlerImpl"
check_impl "ListToolsHandlerImpl" "pub struct ListToolsHandlerImpl" "ListToolsHandlerImpl"
check_impl "CallToolHandlerImpl" "pub struct CallToolHandlerImpl" "CallToolHandlerImpl"
check_impl "ListResourcesHandlerImpl" "pub struct ListResourcesHandlerImpl" "ListResourcesHandlerImpl"
check_impl "ReadResourceHandlerImpl" "pub struct ReadResourceHandlerImpl" "ReadResourceHandlerImpl"
check_impl "ListPromptsHandlerImpl" "pub struct ListPromptsHandlerImpl" "ListPromptsHandlerImpl"
check_impl "GetPromptHandlerImpl" "pub struct GetPromptHandlerImpl" "GetPromptHandlerImpl"
check_impl "CancelledHandlerImpl" "pub struct CancelledHandlerImpl" "CancelledHandlerImpl"

# Factory interfaces
check_impl "McpServerFactory" "pub trait McpServerFactory" "McpServerFactory trait"
check_impl "ToolSchemaFactory" "pub trait ToolSchemaFactory" "ToolSchemaFactory trait"
check_impl "ResourceSchemaFactory" "pub trait ResourceSchemaFactory" "ResourceSchemaFactory trait"
check_impl "PromptSchemaFactory" "pub trait PromptSchemaFactory" "PromptSchemaFactory trait"

# Tests
check_impl_tests "test_mcpserver_" "fn test_mcpserver_" "McpServer tests exist"
check_impl_tests "test_toolregistry_" "fn test_toolregistry_" "ToolRegistry tests exist"

echo ""
echo "═══ Results: $PASS passed, $FAIL failed ═══"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
