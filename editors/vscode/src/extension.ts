// PBLang VS Code Extension v3.0.0
// Connects VS Code to `pblang lsp` (JSON-RPC 2.0 LSP 3.17) with rich SI/Currency dimensional
// hover cards, Tarski CAD / Presburger contract verification badges, Levenshtein quick-fix
// code actions, and canonical `.sapp` document formatting.

import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    const config = vscode.workspace.getConfiguration('pblang');
    const serverPath: string = config.get<string>('serverPath') || 'pblang';
    const strictMode: boolean = config.get<boolean>('strictMode', true);
    const verifyContracts: boolean = config.get<boolean>('verifyContracts', true);

    const lspArgs: string[] = ['lsp'];
    lspArgs.push(strictMode ? '--strict' : '--no-strict');

    const serverOptions: ServerOptions = {
        run: {
            command: serverPath,
            args: lspArgs,
            transport: TransportKind.stdio,
        },
        debug: {
            command: serverPath,
            args: lspArgs,
            transport: TransportKind.stdio,
        },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'pblang' },
            { scheme: 'untitled', language: 'pblang' },
        ],
        initializationOptions: {
            strict: strictMode,
            verifyContracts,
        },
        synchronize: {
            configurationSection: 'pblang',
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{sapp,sap}'),
        },
        middleware: {
            provideHover: async (document, position, token, next) => {
                const hover = await next(document, position, token);
                if (hover && hover.contents) {
                    return hover;
                }
                return undefined;
            },
            provideCodeActions: async (document, range, codeActionContext, token, next) => {
                const actions = await next(document, range, codeActionContext, token);
                return actions ?? [];
            },
            provideDocumentFormattingEdits: async (document, options, token, next) => {
                const edits = await next(document, options, token);
                return edits ?? [];
            },
        },
    };

    client = new LanguageClient(
        'pblang',
        'PBLang 3.0.0 Language Server',
        serverOptions,
        clientOptions
    );

    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBar.text = '$(verified) PBLang 3.0 SI+CAD';
    statusBar.tooltip = 'PBLang 3.0.0 LSP: 9D SI/Currency Units & Tarski CAD / Presburger Contracts Active';
    statusBar.command = 'pblang.check';
    statusBar.show();
    context.subscriptions.push(statusBar);

    // Command: Run current file
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.run', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active PBLang editor');
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const terminal = vscode.window.createTerminal('PBLang Run');
            terminal.show();
            terminal.sendText(`${serverPath} run "${filePath}"`);
        })
    );

    // Command: Static + SI + Contract Check current file
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.check', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active PBLang editor');
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const strictFlag = strictMode ? '--strict' : '';
            const terminal = vscode.window.createTerminal('PBLang Check');
            terminal.show();
            terminal.sendText(`${serverPath} check ${strictFlag} "${filePath}"`);
        })
    );

    // Command: Format current document via LSP formatting provider
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.format', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active PBLang editor');
                return;
            }
            await vscode.commands.executeCommand('editor.action.formatDocument');
        })
    );

    // Command: Generate Markdown documentation for current file
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.doc', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active PBLang editor');
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const terminal = vscode.window.createTerminal('PBLang Docgen');
            terminal.show();
            terminal.sendText(`${serverPath} doc "${filePath}"`);
        })
    );

    // Command: Open Interactive PBLang REPL
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.repl', () => {
            const terminal = vscode.window.createTerminal('PBLang 3.0 REPL');
            terminal.show();
            terminal.sendText(`${serverPath} repl`);
        })
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
