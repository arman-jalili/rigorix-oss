import glob

for f in glob.glob('*/.pi/scripts/ci/check_architecture_conformance.sh'):
    with open(f) as fh:
        content = fh.read()
    
    # Fix find commands
    old1 = 'find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null'
    new1 = 'find . -not -path "*/target/*" -not -path "*/.git/*" \\( -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" \\) 2>/dev/null'
    content = content.replace(old1, new1)
    
    old2 = 'find "$layer_dir" -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null'
    new2 = 'find "$layer_dir" -not -path "*/target/*" -not -path "*/.git/*" \\( -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" \\) 2>/dev/null'
    content = content.replace(old2, new2)
    
    with open(f, 'w') as fh:
        fh.write(content)
    
    # Verify
    import subprocess
    result = subprocess.run(['bash', '-n', f], capture_output=True, text=True)
    if result.returncode == 0:
        print(f'✅ {f}')
    else:
        print(f'❌ {f}: {result.stderr}')
