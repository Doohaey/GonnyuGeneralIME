package.path = os.getenv("GANNYU_RIME_LUA_DIR") .. "/?.lua;" .. package.path
package.preload["gannyu_data_lifecycle"] = function()
  return { current = function(env) return env.data end, init = function() end, fini = function() end }
end
package.preload["gannyu_input_options"] = function()
  return { enabled = function() return true end }
end
package.preload["konny_test_scheme2"] = function()
  return { spellings = { qit = "chit", ne = "ner", ["sï"] = "sii" }, tones = { ["5*"] = "9*" } }
end

local filter = require("gannyu_annotation_filter")
local function run(schema_id, data, comment)
  local env = { engine = { schema = { schema_id = schema_id } }, data = data }
  local input = { iter = function()
    local emitted = false
    return function()
      if emitted then return nil end
      emitted = true
      return { text = "字", comment = comment or "" }
    end
  end }
  local output = {}
  function yield(candidate) table.insert(output, candidate) end
  filter.func(input, env)
  return output[1].comment
end

local annotated = { annotations = { ["字"] = "[文]qit5*" }, readings = {}, synonyms = {} }
assert(run("konny_test_II", annotated) == "[文]chit9*")
assert(run("konny_test_II_gan", annotated) == "[文]chit9*")
assert(run("gannyu_test", annotated) == "[文]qit5*")

local fallback = { annotations = {}, readings = {}, synonyms = {} }
assert(run("konny_test_II", fallback, "Gsï4") == "sii4")
assert(run("konny_test_II_gan", fallback, "Gsï4") == "sii4")
assert(run("gannyu_test", fallback, "Gsï4") == "sï4")
print("scheme2 annotations follow the active scheme")
