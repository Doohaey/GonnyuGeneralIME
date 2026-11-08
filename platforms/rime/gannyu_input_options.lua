local M = {}

local function supported(env)
  local schema = env.engine and env.engine.schema
  if not schema or not schema.config then return true end
  local switches = schema.config:get_list("switches")
  if not switches then return false end
  for index = 0, switches.size - 1 do
    if schema.config:get_string("switches/@" .. index .. "/name") == "gannyu_no_annotations" then
      return true
    end
  end
  return false
end

function M.enabled(env, name)
  local context = supported(env) and env.engine and env.engine.context
  return not context or not context:get_option("gannyu_no_" .. name)
end

local function data_root(root)
  if root:sub(1, 6) == "konny_" then
    return "gannyu_" .. root:sub(7)
  end
  return root
end

function M.names(env)
  local schema_id = env.engine.schema.schema_id
  local ipa = schema_id:sub(-3) == "_II"
  local root = ipa and data_root(schema_id:sub(1, -4)) or schema_id
  if not supported(env) then return schema_id, "" end
  local context = env.engine and env.engine.context
  local mode = context and context:get_option("gannyu_new") and "new"
    or context and context:get_option("gannyu_old") and "old" or "both"
  local relations = M.enabled(env, "relations")
  local dictionary = mode == "both" and relations and "" or "_" .. mode .. (relations and "_rel" or "_norel")
  if ipa then
    local fuzzy = M.enabled(env, "fuzzy")
    local mandarin = fuzzy and M.enabled(env, "mandarin")
    local wenbai = fuzzy and M.enabled(env, "wenbai")
    local suffix = dictionary .. (mandarin and "" or "_gan") .. (wenbai and "" or "_nowenbai")
    return root .. dictionary, suffix
  end
  local full = M.enabled(env, "fuzzy") and M.enabled(env, "fuzzy_rules")
  local mandarin = M.enabled(env, "fuzzy") and M.enabled(env, "mandarin")
  local prism = full and mandarin and "" or (full and "_full" or "_special") .. (mandarin and "_mix" or "_gan")
  local wenbai = M.enabled(env, "fuzzy") and M.enabled(env, "wenbai")
  return schema_id .. dictionary, dictionary .. prism .. (wenbai and "" or "_nowenbai")
end

return M
