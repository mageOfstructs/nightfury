// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import net from 'net';
import process from 'process';
import { access, accessSync, constants } from 'fs';
import { warn } from 'console';

const sockets: { [field: string]: net.Socket } = {};
// TODO: make this support multiple files
let insertLock = false;
let currentlyInRegex = false;

enum RequestType {
  Initialize = 5,
  Revert = 3
}

type InitializeRequest = {
  cc: RequestType.Initialize,
  lang: string
}
type RevertRequest = {
  cc: RequestType.Revert
}
type AdvanceRequest = {
  cc: null,
  text: string
}
type Request = InitializeRequest | AdvanceRequest | RevertRequest;

const Initialize = function(lang: string): InitializeRequest {
  return { cc: 0x05, lang };
};
const Advance = function(text: string): AdvanceRequest {
  return { cc: null, text };
};
const Revert = function(): RevertRequest {
  return { cc: 0x03 };
};

enum ResponseType {
  Ok = 0,
  Error = 1,
  RegexFull = 2,
  CursorHandle = 4,
  InvalidChar = 5,
  RegexStart = 6,
}
type SingleByteResponse = { cc: ResponseType.Ok | ResponseType.RegexFull | ResponseType.RegexStart | ResponseType.InvalidChar };
type OkResponse = { cc: ResponseType.Ok };
type ErrorResponse = { cc: ResponseType.Error, msg: string };
type RegexFullResposne = { cc: ResponseType.RegexFull };
type RegexStartResponse = { cc: ResponseType.RegexStart };
type CursorHandleResponse = { cc: ResponseType.CursorHandle, handle: number };
type InvalidCharResponse = { cc: ResponseType.InvalidChar };
type ExpandedResponse = { cc: null, expanded: string };
type Response = OkResponse | ErrorResponse | RegexFullResposne | RegexStartResponse | CursorHandleResponse | InvalidCharResponse | ExpandedResponse;

function connect(path: string, callback: (socket: net.Socket) => void): net.Socket | null {
  access(path, constants.F_OK, (err) => {
    if (err) {
      console.error("connect: " + err.toString());
    } else {
      const sockopts = {};
      if (process.platform() === "win32") sockopts.port = path;
      else sockopts.path = path;
      const socket = net.createConnection(sockopts, () => {
        vscode.window.showInformationMessage('Connected to Nightfury Server!');
      });
      callback(socket);
    }
  });
  return null;
}

function getLanguage() {
  return getDocument()?.languageId;
}
function getDocument() {
  return vscode.window.activeTextEditor?.document;
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
  return getDocument()?.lineAt(line)?.text.at(character);
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
    if (startChar > 0) { startChar--; }
    else {
      break;
    }
  }
  if (tmp && /\s/.test(tmp)) {
    startChar++;
  }
  while ((tmp = getChar(line, endChar)) && !/\s/.test(tmp)) {
    if (endChar < curLineLen) { endChar++; }
    else {
      break;
    }
  }
  return new vscode.Range(line, startChar, line, endChar);
}

let shortStartOff = 0;
const prevShortStartOffs: number[] = [];
function getTextToReplace(cursorPos: vscode.Position): vscode.Range | undefined {
  const shortStart = vscode.window.activeTextEditor?.document.positionAt(shortStartOff);
  if (!shortStart) { return undefined; }
  if (cursorPos?.isAfterOrEqual(shortStart)) {
    const ret = new vscode.Range(shortStart, cursorPos);
    return ret;
  } else {
    console.warn("cursorPos not after shortStart!");
    console.log(shortStart);
    console.log(cursorPos);
  }
}

function getCursorPos(): vscode.Position | undefined {
  return vscode.window.activeTextEditor?.selection.active;
}
async function removeLastChar() {
  const res = await vscode.window.activeTextEditor?.edit((editBuilder) => {
    const curPos = getCursorPos();
    if (!curPos) { return; }
    editBuilder.delete(new vscode.Range(curPos, curPos.translate(0, 1)));
  });
  if (!res) {
    console.warn("removeLastChar failed!");
  }
}
async function removeText(startPos: vscode.Position) {
  const res = await vscode.window.activeTextEditor?.edit((editBuilder) => {
    const curPos = getCursorPos();
    printObj("startPos", startPos);
    printObj("curPos", curPos);
    if (!curPos || curPos.isBeforeOrEqual(startPos)) {
      console.warn("removeText: invalid positions!");
    }
    console.log("deleting");
    editBuilder.delete(new vscode.Range(startPos, curPos!));
  });
  if (!res) {
    console.warn("removeLastChar failed!");
  }
}
async function insertExpansion(expaned: string, insert: boolean = false) {
  console.log("inserting expansion...");
  const editor = vscode.window.activeTextEditor;
  if (editor) {
    const document = editor.document;
    await editor.edit((editBuilder) => {
      console.log(editor.selections);
      let curPos = editor.selection.active;
      curPos = curPos.translate(0, 1);
      console.log(`shortStart: ${JSON.stringify(document.positionAt(shortStartOff))}`);
      console.log(`curPos: ${JSON.stringify(curPos)}`);
      console.log(document.lineAt(curPos.line).text[curPos.character]);
      const range = getTextToReplace(curPos);
      if (range) {
        if (!insert) {
          console.log(`replacing '${document.getText(range)}' with '${expaned}'`);
          editBuilder.replace(range, expaned);
          console.log(`old shortStartOff: ${shortStartOff}`);
          console.log(`shifting by ${expaned.length}`);
        } else {
          console.log(`inserting '${expaned}'`);
          editBuilder.insert(document.positionAt(shortStartOff), expaned);
        }
        updateShortStartOff(shortStartOff + expaned.length);
        console.log(`new shortStart: ${JSON.stringify(document.positionAt(shortStartOff))}`);
      } else {
        console.warn("Range is undefined!");
      }
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
    case 6:
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
    case 0x5:
    case 0x6:
      return { cc: id! };
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
  const document = vscode.window.activeTextEditor?.document;
  if (document) {
    console.log(document.lineAt(0));
  }
  switch (response.cc) {
    case 0x0:
      if (lastReq?.cc === RequestType.Revert) {
        await revert();
        return;
      }
      if (currentlyInRegex) {
        bumpSSOToCursor();
      }
      console.log("Last request succeeded!");
      return;
    case 0x1:
      console.error(`Server Error: ${response.msg}`);
      return;
    case ResponseType.RegexFull:
      console.log("Regex Full");
      currentlyInRegex = false;
      if (lastReq && !lastReq.cc && document) {
        updateShortStartOff(document.offsetAt(vscode.window.activeTextEditor!.selection.active));
      }
      await insertExpansion(' ', true);
      return;
    case ResponseType.InvalidChar:
      await removeLastChar();
      return;
    case ResponseType.RegexStart:
      currentlyInRegex = true;
      console.log("Regex Start");
      return;
    case null:
      if (lastReq?.cc === RequestType.Revert) {
        console.log(prevShortStartOffs);
        await revert();
        return;
      }
      console.log(`expanding to '${response.expanded}'`);
      await insertExpansion(response.expanded);
  }
}

function printObj(name: string, obj: object | undefined) {
  console.log(`${name}: ${JSON.stringify(obj)}`);
}

async function revert() {
  const document = getDocument()!;
  const ssoPos = document.positionAt(shortStartOff);
  const curPos = getCursorPos()!;
  printObj("ssoPos", ssoPos);
  printObj("curPos", curPos);
  await removeText(document!.positionAt(shortStartOff));
  shortStartOff = prevShortStartOffs.pop() ?? 0;
  console.log("Reverted!");
}

function updateShortStartOff(newSSO: number) {
  prevShortStartOffs.push(shortStartOff);
  shortStartOff = newSSO;
}

function bumpSSOToCursor() {
  const document = getDocument();
  if (!document) {
    console.error("bumpSSOToCursor: document is undefined!");
    return;
  }
  updateShortStartOff(document.offsetAt(vscode.window.activeTextEditor!.selection.active));
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

function isWindows() {
  return process.platform() === "win32";
}

function getSocketPath() {
  if (isWindows()) {
    return "14978";
  }
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
        if (insertLock) { return; } // so we don't trigger on the events we produced
        for (const contentChange of event.contentChanges) {
          console.log(contentChange);
          if (contentChange.rangeLength > 0 && contentChange.text === '') {
            for (let i = 0; i < contentChange.rangeLength; i++) {
              sendRevert();
            }
            continue;
          }
          const textAdded = contentChange.text;
          if (textAdded.length === 0) {
            continue;
          }

          sendChar(textAdded, (err) => {
            if (err) {
              console.error("sendChar callback:");
              console.error(err);
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
    case RequestType.Initialize:
      buf = Buffer.allocUnsafe(req.lang.length + 2); // cc + NUL
      buf.writeUint8(req.cc);
      buf.fill(req.lang, 1, req.lang.length + 1);
      buf.writeUint8(0, req.lang.length + 1);
      return buf;
    case RequestType.Revert:
      buf = Buffer.allocUnsafe(1);
      buf.fill(req.cc);
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
function sendRevert(callback?: ((err?: Error | null) => void) | undefined) {
  const reqObj = Revert();
  send(reqObj, callback);
}

function sendChar(char: string, callback?: ((err?: Error | null) => void) | undefined) {
  // if (char.length > 1) { throw new Error("not a char!"); }

  const reqObj = Advance(char[0]);
  send(reqObj, callback);
}

let lastReq: Request | null = null;
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
