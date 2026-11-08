local users = {}
local M = {}

function M.current(env)
  if not env.engine then return env.data end
  local name = require("gannyu_input_options").names(env) .. "_data"
  if name ~= env.data_name then
    M.fini(env)
    local data = require(name)
    users[name] = (users[name] or 0) + 1
    env.data_name, env.data = name, data
  end
  return env.data
end

function M.init(env)
  local schema_id = env.engine.schema.schema_id
  local root = schema_id:sub(-3) == "_II" and schema_id:sub(1, -4) or schema_id
  if root:sub(1, 6) == "konny_" then
    root = "gannyu_" .. root:sub(7)
  end
  local name = root .. "_data"
  local data = require(name)
  users[name] = (users[name] or 0) + 1
  env.data_name = name
  env.data = data
end

function M.fini(env)
  local name = env.data_name
  if not name then
    return
  end
  local data = env.data
  env.data = nil
  env.data_name = nil
  users[name] = users[name] - 1
  if users[name] == 0 then
    users[name] = nil
    if package.loaded[name] == data then
      package.loaded[name] = nil
    end
    data = nil
    collectgarbage("collect")
  end
end

return M
