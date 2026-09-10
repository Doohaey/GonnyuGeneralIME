local marker = "`"

local M = {}

function M.func(input, segment, env)
  if input ~= marker then
    return
  end
  local best = {}
  for word, codes in pairs(env.data.defaults) do
    for _, code in ipairs(codes) do
      local query_segment = Segment(0, #code)
      query_segment.tags = Set({"abc"})
      local translation = env.translator:query(code, query_segment)
      if translation then
        for candidate in translation:iter() do
          if candidate.text == word then
            candidate.start = segment.start
            candidate._end = segment._end
            local current = best[word]
            if not current or candidate.quality > current.quality then
              candidate.preedit = "\226\128\139"
              best[word] = candidate
            end
            break
          end
        end
      end
    end
  end
  local candidates = {}
  for _, candidate in pairs(best) do
    table.insert(candidates, candidate)
  end
  table.sort(candidates, function(left, right)
    if left.quality == right.quality then
      return left.text < right.text
    end
    return left.quality > right.quality
  end)
  for _, candidate in ipairs(candidates) do
    yield(candidate)
  end
end

function M.init(env)
  env.data = require(env.engine.schema.schema_id .. "_data")
  env.translator = Component.Translator(env.engine, "translator", "script_translator")
end

function M.fini(env)
  env.translator = nil
  env.data = nil
end

return M
