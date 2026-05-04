// pblang VS Code Extension — ED1-alpha
// Starts the pblang LSP server and connects it to VS Code via vscode-languageclient.

import * as vscode from 'vscode';
import * as path from 'path';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    // Path to the pblang LSP server binary.
    // Defaults to 'pblang' on PATH; configurable via setting 'pblang.serverPath'.
    const config = vscode.workspace.getConfiguration('pblang');
    const serverPath: string = config.get('serverPath') || 'pblang';

    const serverOptions: ServerOptions = {
        run: {
            command: serverPath,
            args: ['lsp'],
            transport: TransportKind.stdio,
        },
        debug: {
            command: serverPath,
            args: ['lsp'],
            transport: TransportKind.stdio,
        },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'pblang' },
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{sapp,sap}'),
        },
    };

    client = new LanguageClient(
        'pblang',
        'pblang Language Server',
        serverOptions,
        clientOptions
    );

    // Register the 'pblang: Run current file' command
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.run', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active editor');
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const terminal = vscode.window.createTerminal('pblang run');
            terminal.show();
            terminal.sendText(`pblang run "${filePath}"`);
        })
    );

    // Register the 'pblang: Check current file' command
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.check', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active editor');
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const terminal = vscode.window.createTerminal('pblang check');
            terminal.show();
            terminal.sendText(`pblang --check "${filePath}"`);
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
