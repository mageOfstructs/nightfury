// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import net from 'net';
import process from 'process';
import { access, accessSync, constants } from 'fs';
import { error, warn } from 'console';

let socket: net.Socket | null = null;
let lastInput: String | null = null;
let init: boolean = false;
let knownCapabilities = [];

type InitializeRequest = {
  cc: 0x05,
  lang: string
}
type AdvanceRequest = {
  cc: null,
  text: string
}
type Request = InitializeRequest | AdvanceRequest;

const Initialize = function(lang: string): InitializeRequest {
  return { cc: 0x05, lang };
}
const Advance = function(text: string): AdvanceRequest {
  return { cc: null, text };
}

type OkResponse = { cc: 0x0 };
type ExpandedResponse = { cc: null, expanded: string };
type Response = OkResponse | ExpandedResponse;

function connect(path: string, callback: (socket: net.Socket) => void): net.Socket | null {
  access(path, constants.F_OK, (err) => {
    if (err) {
      console.error("connect: " + err.toString());
    } else {
      socket = net.createConnection(path, () => {
        vscode.window.showInformationMessage('Connected to Nightfury Server!');
      });
      callback(socket);
      return socket;
    }
  });
  return null;
}

function getLanguage() {
  return vscode.window.activeTextEditor?.document.languageId;
}

const socketSetup = (socket: net.Socket) => {
  socket.addListener('data', (data) => {
    console.log("Response from server:");
    const rawData = data.toString();
    console.log(rawData);
    console.log("parsing...");
    handleResponse(parseResponse(data));
  });
  sendInit(getLanguage()!);
};

function insertExpansion(expaned: String) {
  console.log("inserting expansion...");
  const editor = vscode.window.activeTextEditor;
  if (editor) {
    const document = editor.document;
    editor.edit((editBuilder) => {
      const curPos = editor.selection.active;
      const range = document.getWordRangeAtPosition(curPos);
      // hack to keep userdefineds from being overwritten
      // probably need a new protocol message saying the userdefined was completed
      if (range && expaned.startsWith(document.getText(range))) {
        editBuilder.replace(range, expaned + " ");
      } else if (!range) {
        console.warn("Range is undefined!");
      }
    });
  }
}

function parseResponse(raw: Buffer): Response {
  let ret: Response;
  switch (raw.at(0)) {
    case 0x0:
      ret = { cc: 0x0 };
      return ret;
    default:
      ret = { cc: null, expanded: raw.toString('utf8', 0, raw.length - 1) };
      return ret;
  }
}
function handleResponse(response: Response) {
  if (response.cc === 0x0) {
    console.log("Last request succeeded!");
    return;
  }
  if (response.expanded) {
    insertExpansion(response.expanded);
  }
  // if (response.Capabilities) {
  //   knownCapabilities = response.Capabilities;
  // } else if (response.Expanded) {
  //   insertExpansion(response.Expanded);
  // }
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

  const disposableActivateNightfury = vscode.commands.registerCommand('nightfury-vscode.activateForCurrent', () => {
    if (vscode.window.activeTextEditor?.document.languageId) {
      connect(getSocketPath(), socketSetup);
      vscode.workspace.onDidChangeTextDocument(function(event) {
        for (const contentChange of event.contentChanges) {
          const textAdded = contentChange.text.trim();
          if (textAdded.length === 0) {
            continue;
          }

          sendChar(textAdded, (err) => {
            if (err) {
              error("sendChar callback:");
              console.error(err);
            } else {
              lastInput = textAdded;
            }
          });
        }
      });
    } else {
      vscode.window.showInformationMessage("Can't determine language!");
    }
  });


  context.subscriptions.push(disposableGetCaps);
  context.subscriptions.push(disposableActivateNightfury);
}

function buildRequest(req: Request) {
  let buf;
  switch (req.cc) {
    case 0x05:
      buf = Buffer.allocUnsafe(req.lang.length + 2); // cc + NUL
      buf.writeUint8(req.cc);
      buf.fill(req.lang, 1, req.lang.length + 1);
      buf.writeUint8(0, req.lang.length + 1);
      return buf;
    default:
      buf = Buffer.allocUnsafe(req.text.length + 1);
      buf.fill(req.text, 0, req.text.length);
      buf.writeUint8(0, req.text.length);
      return buf;
  }
}

function sendInit(name: string, callback?: ((err?: Error | null) => void) | undefined) {
  const reqObj = Initialize(name);
  send(reqObj, callback);
}

function sendChar(char: string, callback?: ((err?: Error | null) => void) | undefined) {
  if (char.length > 1) { throw new Error("not a char!"); }
  if (!init) {
    sendInit(vscode.window.activeTextEditor!.document.languageId, () => init = true);
  }

  const reqObj = Advance(char);
  send(reqObj, callback);
}

let lastReq = {};
function send(req: Request, callback?: ((err?: Error | null) => void) | undefined) {
  lastReq = req;
  const buf = buildRequest(req);
  console.log(buf);
  if (socket) {
    socket?.write(buf, callback);
  } else {
    console.error("Socket not connected!");
  }
}

// This method is called when your extension is deactivated
export function deactivate() {
  socket?.destroy();
  socket = null;
}
