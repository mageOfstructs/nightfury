// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import net from 'net';
import process from 'process';
import { access, accessSync, constants } from 'fs';
import { warn } from 'console';

let socket: net.Socket | null = null;
let lastInput: String | null = null;

function connect(path: string, callback: (socket: net.Socket) => void) {
  access(path, constants.F_OK, (err) => {
    if (err) {
      console.error("connect: " + err.toString());
    } else {
      socket = net.createConnection(path, () => {
        vscode.window.showInformationMessage('Connected to Nightfury Server!');
      });
      callback(socket);
    }
  });
}

const socketSetup = (socket: net.Socket) => {
  socket.addListener('data', (data) => {
    console.log("Response from server:");
    const rawData = data.toString();
    console.log(rawData);
    console.log("parsing...");
    const parsedData = JSON.parse(rawData.substring(0, rawData.length - 1));
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
          } else {
            console.warn("Range is undefined!");
          }
        })
      }
    }
  });
}

function getRuntimeDir(defaultDir?: string) {
  if (typeof process.env["XDG_RUNTIME_DIR"] === "string") {
    return process.env["XDG_RUNTIME_DIR"];
  }
  try {
    const rtdDefault = "/run/user/" + process.env["EUID"];
    accessSync(rtdDefault, constants.F_OK);
    return rtdDefault;
  } catch (err) {
    warn("No runtime dir! Using default...");
  }
  return defaultDir;
}

function getSocketPath() {
  return getRuntimeDir('/home/jason/clones/nightfury/nightfury-server') + '/nightfury.sock';
}

export function activate(context: vscode.ExtensionContext) {
  connect('/home/jason/clones/nightfury/nightfury-server/nightfury.sock', socketSetup);

  // The command has been defined in the package.json file
  // Now provide the implementation of the command with registerCommand
  // The commandId parameter must match the command field in package.json
  const disposableGetCaps = vscode.commands.registerCommand('nightfury-vscode.getCapabilities', () => {
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
    });
  }

  context.subscriptions.push(disposableGetCaps);
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
