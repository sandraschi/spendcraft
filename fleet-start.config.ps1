# Per-repo fleet start config for spendcraft
# Assigned ports in mcp-central-docs/operations/WEBAPP_PORTS.md
@{
    Name         = 'spendcraft'
    FrontendPort = 11262
    BackendPort  = 11263
    HealthPath   = '/health'
    Backend = @{
        Kind       = 'custom'
        Command    = 'cargo run -p spendcraft-mcp -- --mcp'
        WorkingDir = '.'
    }
    Frontend = @{
        Kind       = 'custom'
        Command    = 'cargo run -p spendcraft-app'
        WorkingDir = '.'
    }
}
