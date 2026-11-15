// PBLang VS Code Extension v4.2.0
// Connects VS Code to the pure-PBLang v4.2.0 Multi-File Workspace LSP (JSON-RPC 2.0 LSP 3.17)
// with cross-file go-to-definition, workspace rename & references, signatureHelp with
// requires/ensures contracts, 9D SI/Currency & SQL/exhaustive inlayHints, coverage reporting,
// and coverage-guided shrinking property fuzzing.

import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

function resolveWorkspaceDevtoolsScript(scriptRelPath: string): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (folders) {
        for (const folder of folders) {
            const candidate = path.join(folder.uri.fsPath, scriptRelPath);
            if (fs.existsSync(candidate)) {
                return candidate;
            }
        }
    }
    return undefined;
}

export function activate(context: vscode.ExtensionContext): void {
    const config = vscode.workspace.getConfiguration('pblang');
    const serverPath: string = config.get<string>('serverPath') || 'pblang';
    const strictMode: boolean = config.get<boolean>('strictMode', true);
    const verifyContracts: boolean = config.get<boolean>('verifyContracts', true);
    const strictSql: boolean = config.get<boolean>('strictSql', false);
    const inlayHintsEnabled: boolean = config.get<boolean>('lsp.inlayHints', true);
    const useWorkspaceV42: boolean = config.get<boolean>('lsp.useWorkspaceServerV42', true);

    const workspaceLspScript = useWorkspaceV42
        ? resolveWorkspaceDevtoolsScript(path.join('stdlib', 'devtools', 'workspace_lsp_v42.sapp'))
        : undefined;

    const lspArgs: string[] = workspaceLspScript
        ? ['run', workspaceLspScript, '--allow-all', '--', '--lsp-v42-stdio']
        : ['lsp', strictMode ? '--strict' : '--no-strict'];

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
            strictSql,
            verifyContracts,
            inlayHints: inlayHintsEnabled,
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
            provideInlayHints: async (document, viewPort, token, next) => {
                const liveCfg = vscode.workspace.getConfiguration('pblang');
                if (!liveCfg.get<boolean>('lsp.inlayHints', true)) {
                    return [];
                }
                const hints = await next(document, viewPort, token);
                return hints ?? [];
            },
            provideSignatureHelp: async (document, position, sigContext, token, next) => {
                return next(document, position, sigContext, token);
            },
            provideRenameEdits: async (document, position, newName, token, next) => {
                return next(document, position, newName, token);
            },
            provideReferences: async (document, position, refContext, token, next) => {
                const refs = await next(document, position, refContext, token);
                return refs ?? [];
            },
        },
    };

    client = new LanguageClient(
        'pblang',
        'PBLang 4.2.0 Workspace Language Server',
        serverOptions,
        clientOptions
    );

    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBar.text = '$(verified) PBLang 4.2 LSP+SQL+SI';
    statusBar.tooltip = 'PBLang 4.2.0 Workspace LSP: Multi-File Refactoring, Algebraic/Trait/SQL Verifier, 9D SI/EUR Units & CAD Contracts';
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

    // Command: Static + SI + Contract + SQL Check current file
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
            const terminal = vscode.window.createTerminal('PBLang 4.2 REPL');
            terminal.show();
            terminal.sendText(`${serverPath} repl`);
        })
    );

    // Command: Generate Line & Branch Coverage Report (Pure PBLang DevTools)
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.testCoverage', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active PBLang editor');
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const covScript =
                resolveWorkspaceDevtoolsScript(path.join('stdlib', 'devtools', 'coverage_and_fuzzer.sapp')) ||
                'stdlib/devtools/coverage_and_fuzzer.sapp';
            const terminal = vscode.window.createTerminal('PBLang Coverage');
            terminal.show();
            terminal.sendText(`${serverPath} run "${covScript}" --allow-all -- --coverage-cli "${filePath}"`);
        })
    );

    // Command: Fuzz Current File with Coverage-Guided Shrinking (Pure PBLang DevTools)
    context.subscriptions.push(
        vscode.commands.registerCommand('pblang.fuzzCurrentFile', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active PBLang editor');
                return;
            }
            const filePath = editor.document.uri.fsPath;
            const fuzzScript =
                resolveWorkspaceDevtoolsScript(path.join('stdlib', 'devtools', 'coverage_and_fuzzer.sapp')) ||
                'stdlib/devtools/coverage_and_fuzzer.sapp';
            const terminal = vscode.window.createTerminal('PBLang Fuzzer');
            terminal.show();
            terminal.sendText(`${serverPath} run "${fuzzScript}" --allow-all -- --fuzz-cli "${filePath}"`);
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
