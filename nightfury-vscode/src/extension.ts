// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import net from 'net';

let socket: net.Socket | null = null;
let lastInput: String | null = null;

// This method is called when your extension is activated
// Your extension is activated the very first time the command is executed
export function activate(context: vscode.ExtensionContext) {
	socket = net.createConnection('/home/jason/clones/nightfury/nightfury-server/nightfury.sock');
	socket?.addListener('data', (data) => {
		console.log("Response from server:");
		const rawData = data.toString();
		console.log(rawData);
		console.log("parsing...");
		const parsedData = JSON.parse(rawData.substring(0, rawData.length-1));
		console.log(parsedData);
		if (parsedData["Expanded"]) {
			console.log("inserting expansion...");
			const editor = vscode.window.activeTextEditor;
			if (editor) {
				const document = editor.document;
				editor.edit((editBuilder) => {
					const curPos = editor.selection.active;
					const range = document.getWordRangeAtPosition(curPos);
					if (range) {
						editBuilder.replace(range, parsedData["Expanded"] + " ");
					}
				})
			}
		}
	});

	// Use the console to output diagnostic information (console.log) and errors (console.error)
	// This line of code will only be executed once when your extension is activated
	console.log('Congratulations, your extension "nightfury-vscode" is now active!');

	// The command has been defined in the package.json file
	// Now provide the implementation of the command with registerCommand
	// The commandId parameter must match the command field in package.json
	const disposable = vscode.commands.registerCommand('nightfury-vscode.helloWorld', () => {
		// The code you place here will be executed every time your command is executed
		// Display a message box to the user
		vscode.window.showInformationMessage('Rawrawrawrawrawr from nightfury-vscode!');
		console.log("Current language: " + vscode.window.activeTextEditor?.document.languageId);
		let buf = Buffer.from("\"GetCapabilities\"\0");
		socket?.write(buf, (err) => {
			if (err) {
				console.error(err);
			}
			console.log("Request sent!");

		});
	});

	if (vscode.window.activeTextEditor?.document.languageId) {
		sendInit(vscode.window.activeTextEditor?.document.languageId);
	}
	vscode.workspace.onDidChangeTextDocument(function(event) {
		for (const contentChange of event.contentChanges) {
			const textAdded = contentChange.text.trim();
			if (textAdded.length == 0) {
				continue;
			}

			sendChar(textAdded, (err) => {
				if (err) {
					console.error(err);
				} else {
					lastInput = textAdded;
				}
			});
		}
		console.log(event);
	});

	context.subscriptions.push(disposable);
}

function buildRequest(req: Object | String) {
	const jsonStr = JSON.stringify(req) + '\0';
	console.log("Request: " + jsonStr);
	return Buffer.from(jsonStr);
}

// TODO: make a cool TS Enum out of all this
function sendInit(name: String, callback?: ((err?: Error | null) => void) | undefined) {
	const reqObj = { "Init": name };
	const buf = buildRequest(reqObj);
	socket?.write(buf, callback);
}

function sendChar(char: String, callback?: ((err?: Error | null) => void) | undefined) {
	if (char.length > 1) throw new Error("not a char!");

	const reqObj = { "Advance": char };
	const buf = buildRequest(reqObj);
	socket?.write(buf, callback);
}

// This method is called when your extension is deactivated
export function deactivate() {
	socket?.destroy();
	socket = null;
}
