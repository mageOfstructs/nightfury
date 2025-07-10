// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import net from 'net';
import process from 'process';
import { access, accessSync, constants } from 'fs';
import { error, warn } from 'console';

const sockets: { [field: string]: net.Socket } = {};
let lastInput: String | null = null;
let insertLock = false;

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
};
const Advance = function(text: string): AdvanceRequest {
  return { cc: null, text };
};

type OkResponse = { cc: 0x0 };
type ErrorResponse = { cc: 0x1, msg: string };
type RegexFullResposne = { cc: 0x2 };
type CursorHandleResponse = { cc: 0x4, handle: number };
type ExpandedResponse = { cc: null, expanded: string };
type Response = OkResponse | ErrorResponse | RegexFullResposne | CursorHandleResponse | ExpandedResponse;

function connect(path: string, callback: (socket: net.Socket) => void): net.Socket | null {
  access(path, constants.F_OK, (err) => {
    if (err) {
      console.error("connect: " + err.toString());
    } else {
      const socket = net.createConnection(path, () => {
        vscode.window.showInformationMessage('Connected to Nightfury Server!');
      });
      callback(socket);
    }
  });
  return null;
}

function getLanguage() {
  return vscode.window.activeTextEditor?.document.languageId;
}

function handleMsgs(msgs: Buffer[]) {
  insertLock = true;
  const msg = msgs.shift();
  if (msg) {
    handleResponse(parseResponse(msg)).then(() => handleMsgs(msgs));
  } else {
    insertLock = false;
  }
}

const socketSetup = (socket: net.Socket) => {
  socket.addListener('data', (data) => {
    console.log("Response from server:");
    console.log(data);
    console.log("splitting...");
    handleMsgs(splitBufIntoMessages(data));
  });
  const path = vscode.window.activeTextEditor!.document.uri.path;
  sockets[path] = socket;
  sendInit(getLanguage()!);
};

function getChar(line: number, character: number): string | undefined {
  return vscode.window.activeTextEditor?.document.lineAt(line)?.text.at(character);
}
function getWordRangeAtPosition(pos: vscode.Position): vscode.Range | undefined {
  const curLineLen = vscode.window.activeTextEditor?.document.lineAt(pos.line).text.length;
  if (!curLineLen) {
    return undefined;
  }
  let startChar, endChar;
  startChar = endChar = pos.character;
  let line = pos.line;
  let tmp;
  while ((tmp = getChar(line, startChar)) && !/\s/.test(tmp)) {
    if (startChar > 0) startChar--;
    else {
      break;
    }
  }
  if (tmp && /\s/.test(tmp)) {
    startChar++;
  }
  while ((tmp = getChar(line, endChar)) && !/\s/.test(tmp)) {
    if (endChar < curLineLen) endChar++;
    else {
      break;
    }
  }
  return new vscode.Range(line, startChar, line, endChar);
}

async function insertExpansion(expaned: string, insert: boolean = false) {
  console.log("inserting expansion...");
  const editor = vscode.window.activeTextEditor;
  if (editor) {
    const document = editor.document;
    await editor.edit((editBuilder) => {
      console.log(editor.selections);
      let curPos = editor.selection.active;
      if (document.lineAt(curPos.line).text.length <= curPos.character) {
        curPos = curPos.translate(0, curPos.character - document.lineAt(curPos.line).text.length - 1);
      }
      console.log(curPos);
      console.log(document.lineAt(curPos.line).text[curPos.character]);
      const range = getWordRangeAtPosition(curPos);
      console.log(range);
      if (range) {
        if (!insert) {
          console.log(`replacing '${document.getText(range)}'`);
          editBuilder.replace(range, expaned + " ");
        } else {
          console.log(`inserting '${expaned}'`);
          editBuilder.insert(curPos, expaned);
        }
      } else {
        console.warn("Range is undefined!");
        // editBuilder.replace(editor.selection, expaned + " ");
      }
      // if (range && expaned.startsWith(document.getText(range))) {
      //   if (!insert) {
      //     editBuilder.replace(range, expaned + " ");
      //   }
      // } else if (range) {
      //   console.log(curPos);
      // } else {
      //   console.warn("Range is undefined!");
      // }
    });
  } else {
    console.warn("editor is undefined!");
  }
}

function isSingleByteResponse(respId: number): boolean {
  switch (respId) {
    case 0:
    case 2:
    case 5:
      return true;
    default:
      return false;
  }
}

function splitBufIntoMessages(raw: Buffer): Buffer[] {
  const messages = [];
  let prev = 0;
  let buf;
  for (let i = 0; i < raw.length; i++) {
    const curByte = raw.at(i)!;
    switch (true) {
      case curByte === 0x0:
        buf = Buffer.allocUnsafe(i - prev + 1);
        raw.copy(buf, 0, prev, i + 1);
        prev = i + 1;
        messages.push(buf);
        break;
      case curByte === 0x4:
        buf = Buffer.allocUnsafe(2);
        raw.copy(buf, 0, i, (i + 2));
        prev = i + 2;
        i++;
        messages.push(buf);
        break;
      case isSingleByteResponse(curByte):
        buf = Buffer.allocUnsafe(1);
        raw.copy(buf, 0, i, i + 1);
        prev = i + 1;
        messages.push(buf);
        break;
      case curByte < 0x8:
        prev = i;
    }
  }
  console.log("messages:");
  messages.forEach(msg => console.log('\t', msg));
  return messages;
}
function parseResponse(raw: Buffer): Response {
  let ret: Response;
  const id = raw.at(0);
  switch (id) {
    case 0x0:
    case 0x2:
      ret = { cc: id! };
      return ret;
    case 0x1:
      return { cc: id!, msg: raw.toString('utf8', 1, raw.length - 1) };
    case 0x4:
      return { cc: id!, handle: raw.at(1)! };
    default:
      ret = { cc: null, expanded: raw.toString('utf8', 0, raw.length - 1) };
      return ret;
  }
}
async function handleResponse(response: Response) {
  console.log(vscode.window.activeTextEditor?.document.lineAt(0));
  switch (response.cc) {
    case 0x0:
      console.log("Last request succeeded!");
      return;
    case 0x1:
      console.error(`Server Error: ${response.msg}`);
      return;
    case 0x2:
      console.log("inserting space");
      await insertExpansion(' ', true);
      return;
    case null:
      console.log(`inserting '${response.expanded}'`);
      await insertExpansion(response.expanded);
  }
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

function getCurrentSockConn(): net.Socket | null {
  const path = vscode.window.activeTextEditor?.document.uri.path;
  if (path) {
    return sockets[path];
  }
  return null;
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
    let buf = Buffer.from("\x01");
    getCurrentSockConn()?.write(buf, (err) => {
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
        if (insertLock) return;
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

  const reqObj = Advance(char);
  send(reqObj, callback);
}

let lastReq = {};
function send(req: Request, callback?: ((err?: Error | null) => void) | undefined) {
  lastReq = req;
  const buf = buildRequest(req);
  console.log(buf);
  const socket = getCurrentSockConn();
  if (socket) {
    socket?.write(buf, callback);
  } else {
    console.error("Socket not connected!");
  }
}

// This method is called when your extension is deactivated
export function deactivate() {
  for (const socket of Object.values(sockets)) {
    socket.destroy();
  }
}
