"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.normalizeTrace = normalizeTrace;
exports.isGodotDocument = isGodotDocument;
exports.activateWithDeps = activateWithDeps;
exports.activate = activate;
exports.deactivate = deactivate;
exports.resetExtensionStateForTests = resetExtensionStateForTests;
const child_process_1 = require("child_process");
let client;
let starting = false;
function normalizeTrace(value) {
    if (value === 'messages' || value === 'verbose') {
        return value;
    }
    return 'off';
}
function commandExists(command) {
    const lookupCommand = process.platform === 'win32' ? 'where' : 'which';
    const result = (0, child_process_1.spawnSync)(lookupCommand, [command], { encoding: 'utf8' });
    return result.status === 0;
}
async function hasGodotWorkspaceFiles(workspace) {
    const excludes = '**/{.git,node_modules,target,out,dist}/**';
    const [gd, gdshader] = await Promise.all([
        workspace.findFiles('**/*.gd', excludes, 1),
        workspace.findFiles('**/*.gdshader', excludes, 1),
    ]);
    return gd.length > 0 || gdshader.length > 0;
}
function isGodotDocument(doc) {
    if (!doc) {
        return false;
    }
    if (doc.languageId === 'gdscript' || doc.languageId === 'gdshader') {
        return true;
    }
    return doc.fileName.endsWith('.gd') || doc.fileName.endsWith('.gdshader');
}
function createDefaultDeps() {
    const vscodeMod = require('vscode');
    const languageClientMod = require('vscode-languageclient/node');
    return {
        workspace: vscodeMod.workspace,
        window: vscodeMod.window,
        commands: vscodeMod.commands,
        uri: vscodeMod.Uri,
        commandExists,
        createLanguageClient: (id, name, serverOptions, clientOptions) => new languageClientMod.LanguageClient(id, name, serverOptions, clientOptions),
        stdioTransport: languageClientMod.TransportKind.stdio,
    };
}
async function activateWithDeps(context, deps) {
    if (starting) {
        return;
    }
    starting = true;
    try {
        const cfg = deps.workspace.getConfiguration('gozen');
        const enabled = cfg.get('enable', true);
        const outputChannel = deps.window.createOutputChannel('Gozen LSP');
        context.subscriptions.push(outputChannel);
        const restartCmd = deps.commands.registerCommand('gozen.restartLanguageServer', async () => {
            if (!client) {
                void deps.window.showInformationMessage('Gozen language server is not running. Open a .gd or .gdshader file to start it.');
                return;
            }
            await client.stop();
            await client.start();
        });
        context.subscriptions.push(restartCmd);
        if (!enabled) {
            outputChannel.appendLine('Gozen extension is disabled by setting "gozen.enable".');
            return;
        }
        const command = cfg.get('path', 'gozen');
        if (!deps.commandExists(command)) {
            outputChannel.appendLine(`Could not resolve Gozen executable: ${command}`);
            const openSettings = 'Open Settings';
            const viewReadme = 'View README';
            const choice = await deps.window.showErrorMessage(`Gozen executable not found: "${command}". Configure "gozen.path" or add "gozen" to PATH.`, openSettings, viewReadme);
            if (choice === openSettings) {
                await deps.commands.executeCommand('workbench.action.openSettings', 'gozen.path');
            }
            else if (choice === viewReadme) {
                const readmeUri = deps.uri.joinPath(context.extensionUri, 'README.md');
                await deps.commands.executeCommand('markdown.showPreview', readmeUri);
            }
            return;
        }
        const hasWorkspaceFiles = await hasGodotWorkspaceFiles(deps.workspace);
        if (!hasWorkspaceFiles && !isGodotDocument(deps.window.activeTextEditor?.document)) {
            outputChannel.appendLine('No Godot files found in workspace. Skipping Gozen LSP startup.');
            return;
        }
        const serverOptions = {
            run: {
                command,
                args: ['lsp'],
                transport: deps.stdioTransport,
            },
            debug: {
                command,
                args: ['lsp'],
                transport: deps.stdioTransport,
                options: {
                    env: {
                        ...process.env,
                        RUST_LOG: process.env.RUST_LOG || 'info',
                    },
                },
            },
        };
        const clientOptions = {
            documentSelector: [
                { scheme: 'file', language: 'gdscript' },
                { scheme: 'file', pattern: '**/*.gd' },
                { scheme: 'file', pattern: '**/*.gdshader' },
            ],
            outputChannel,
            synchronize: {
                fileEvents: deps.workspace.createFileSystemWatcher('**/*.{gd,gdshader,tscn,tres,godot,json}'),
            },
        };
        if (!client) {
            client = deps.createLanguageClient('gozen', 'Gozen Language Server', serverOptions, clientOptions);
        }
        void client.setTrace(normalizeTrace(cfg.get('trace.server', 'off')));
        await client.start();
        outputChannel.appendLine(`Started Gozen LSP using command: ${command} lsp`);
        const disposable = deps.workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration('gozen.trace.server') && client) {
                const trace = deps.workspace.getConfiguration('gozen').get('trace.server', 'off');
                void client.setTrace(normalizeTrace(trace));
                outputChannel.appendLine(`Updated server trace level: ${trace}`);
            }
        });
        context.subscriptions.push(disposable);
    }
    finally {
        starting = false;
    }
}
async function activate(context) {
    await activateWithDeps(context, createDefaultDeps());
}
async function deactivate() {
    if (!client) {
        return;
    }
    await client.stop();
    client = undefined;
}
function resetExtensionStateForTests() {
    client = undefined;
    starting = false;
}
//# sourceMappingURL=extension.js.map