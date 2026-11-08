local function clean_spelling(comment)
  return (comment or ""):gsub("^G", ""):gsub(" G", " ")
end

local function is_letter(cp)
  return (cp >= 65 and cp <= 90) or (cp >= 97 and cp <= 122)
    or cp == 0xEA or cp == 0xEF or cp == 0xF4
end

local function convert_text(text, display)
  if not display or not text or text == "" then
    return text
  end
  local output = {}
  local length = #text
  local index = 1
  local function next_offset(position)
    local next_position = utf8.offset(text, 2, position)
    return next_position or length + 1
  end
  while index <= length do
    local codepoint = utf8.codepoint(text, index)
    if not is_letter(codepoint) then
      local next_index = next_offset(index)
      output[#output + 1] = text:sub(index, next_index - 1)
      index = next_index
    else
      local start = index
      local next_index = next_offset(index)
      while next_index <= length and is_letter(utf8.codepoint(text, next_index)) do
        index = next_index
        next_index = next_offset(index)
      end
      local body_end = next_index - 1
      while next_index <= length do
        local char = text:sub(next_index, next_index)
        if char:match("%d") or char == "*" then
          index = next_index
          next_index = next_offset(index)
        else
          break
        end
      end
      local body = text:sub(start, body_end)
      local token = text:sub(start, index)
      local scheme2 = display.spellings and display.spellings[body]
      if scheme2 then
        local tone = token:sub(#body + 1)
        if tone ~= "" then
          tone = (display.tones and display.tones[tone]) or tone
        end
        output[#output + 1] = scheme2 .. tone
      else
        output[#output + 1] = token
      end
      index = next_index
    end
  end
  return table.concat(output)
end

local displays = {}

local function display_for(env)
  local schema_id = env.engine and env.engine.schema and env.engine.schema.schema_id
  if not schema_id then
    return nil
  end
  local root = schema_id:match("^(.-)_II$") or schema_id:match("^(.-)_II_")
  if not root then
    return nil
  end
  if displays[schema_id] ~= nil then
    return displays[schema_id] or nil
  end
  local ok, display = pcall(require, root .. "_scheme2")
  if ok and type(display) == "table" then
    displays[schema_id] = display
    return display
  end
  displays[schema_id] = false
  return nil
end

local function sentence_reading(text, data, display)
  local readings = {}
  for character in text:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
    local reading = data.readings[character]
    if not reading then
      return nil
    end
    table.insert(readings, convert_text(reading, display))
  end
  return table.concat(readings, " ")
end

local M = {}

function M.func(input, env)
  local lifecycle = require("gannyu_data_lifecycle")
  local data = lifecycle.current(env)
  local display = display_for(env)
  local enabled = require("gannyu_input_options").enabled(env, "annotations")
  for candidate in input:iter() do
    local annotation = data.annotations[candidate.text]
    if annotation then
      annotation = convert_text(annotation, display)
    end
    candidate.comment = enabled and (annotation
      or sentence_reading(candidate.text, data, display)
      or convert_text(clean_spelling(candidate.comment), display)) or ""
    if enabled and data.synonyms and require("gannyu_input_options").enabled(env, "synonyms") then
      local words = data.synonyms[candidate.text]
      if words and #words > 0 then
        candidate.comment = candidate.comment .. (candidate.comment == "" and "" or " ") .. "[联]" .. table.concat(words, "/")
      end
    end
    yield(candidate)
  end
end

local lifecycle = require("gannyu_data_lifecycle")
M.init = lifecycle.init
M.fini = lifecycle.fini

return M
