local M = {}

-- local unistd = require("posix.unistd");
local socket = require("posix.sys.socket");

local night_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM, 0)
local xdgrtd = os.getenv("XDG_RUNTIME_DIR") or ("/run/user/" .. os.getenv("EUID"));
local conn_res = socket.connect(night_socket, { family = socket.AF_UNIX, path = xdgrtd .. "/nightfury.sock" })
if conn_res ~= 0 then
  print("xdgrtd: " .. xdgrtd)
  print("Couldn't connect: " .. tostring(conn_res))
  return nil
end

local function read_until_null()
  local ret = ""
  local tmp = socket.recv(night_socket, 1)
  while tmp ~= "\0" do
    ret = ret .. tmp
    tmp = socket.recv(night_socket, 1)
  end
  if type(tmp) == "string" and tmp ~= "\0" then
    print("Error while reading: " .. tmp)
    return nil
  end
  return ret
end

function M.getCapabilities()
  local msg = "\"GetCapabilities\"\0"
  if socket.send(night_socket, msg) ~= 18 then
    print("Couldn't send message!")
  end

  local res = read_until_null()
  if res then
    print(res)
  end
end

return M
