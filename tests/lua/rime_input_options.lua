package.path = os.getenv("GANNYU_RIME_LUA_DIR") .. "/?.lua;" .. package.path
local options = require("gannyu_input_options")
local function environment(names, values)
  return {engine = {
    schema = {schema_id = "gannyu_test", config = {
      get_list = function(_, key) assert(key == "switches"); return {size = #names} end,
      get_string = function(_, key)
        local index = assert(key:match("switches/@(%d+)/name"))
        return names[tonumber(index) + 1]
      end,
    }},
    context = {get_option = function(_, key) return values[key] == true end},
  }}
end
for _, names in ipairs({{"ascii_mode", "gannyu_no_annotations"}, {"gannyu_no_fuzzy", "ascii_mode", "gannyu_no_annotations"}}) do
  local env = environment(names, {gannyu_no_fuzzy = true, gannyu_old = true})
  local dictionary, suffix = options.names(env)
  assert(dictionary == "gannyu_test_old_rel")
  assert(suffix == "_old_rel_special_gan_nowenbai")
  env.engine.context = {get_option = function(_, key) return key == "gannyu_no_annotations" end}
  dictionary, suffix = options.names(env)
  assert(dictionary == "gannyu_test" and suffix == "")
  assert(not options.enabled(env, "annotations"))
end
local legacy = environment({"ascii_mode"}, {gannyu_no_annotations = true, gannyu_old = true})
local dictionary, suffix = options.names(legacy)
assert(dictionary == "gannyu_test" and suffix == "")
assert(options.enabled(legacy, "annotations"))
for _, annotations in ipairs({false, true}) do
  for _, relations in ipairs({false, true}) do
    for _, fuzzy in ipairs({false, true}) do
      for _, wenbai in ipairs({false, true}) do
        local env = environment({"gannyu_no_annotations"}, {
          gannyu_no_annotations = not annotations,
          gannyu_no_relations = not relations,
          gannyu_no_fuzzy = not fuzzy,
          gannyu_no_wenbai = not wenbai,
        })
        local dictionary, suffix = options.names(env)
        assert(dictionary == (relations and "gannyu_test" or "gannyu_test_both_norel"))
        assert((suffix:match("_nowenbai$") ~= nil) == not (fuzzy and wenbai))
      end
    end
  end
end


local ipa = environment({"gannyu_no_annotations"}, {
  gannyu_old = true,
  gannyu_no_mandarin = true,
  gannyu_no_wenbai = true,
})
ipa.engine.schema.schema_id = "gannyu_test_ipa"
local ipa_dictionary, ipa_suffix = options.names(ipa)
assert(ipa_dictionary == "gannyu_test_old_rel")
assert(ipa_suffix == "_old_rel_gan_nowenbai")
ipa.engine.context = {get_option = function(_, key) return key == "gannyu_both" end}
ipa_dictionary, ipa_suffix = options.names(ipa)
assert(ipa_dictionary == "gannyu_test" and ipa_suffix == "")

print("switch declarations control hot options without an additional marker")
