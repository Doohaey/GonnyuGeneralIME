local marker = "`"

local function is_mobile_rime()
  local distribution = (rime_api:get_distribution_code_name() or ""):lower()
  if distribution == "trime" or distribution == "hamster" or distribution == "irime" then
    return true
  end
  local user_data = (rime_api:get_user_data_dir() or ""):lower()
  return user_data:find("/data/user/", 1, true)
    or user_data:find("/data/data/", 1, true)
    or user_data:find("/var/mobile/", 1, true)
end

local function arm(env, context)
  context = context or env.engine.context
  if not env.suppress and not context:get_option("ascii_mode") and context.input == "" then
    context:push_input(marker)
  end
end

local function clear_marker(env, context)
  if context.input ~= marker then
    return
  end
  env.suppress = true
  context:clear()
  env.suppress = false
end

local M = {}

function M.func(key, env)
  local context = env.engine.context
  if context.input ~= marker or key:release() then
    return 2
  end
  local repr = key:repr()
  if repr:match("^[0-9]$")
    or repr == "Left"
    or repr == "Right"
    or repr == "Up"
    or repr == "Down"
    or repr == "Page_Up"
    or repr == "Page_Down" then
    return 2
  end
  clear_marker(env, context)
  return 2
end

function M.init(env)
  if not is_mobile_rime() then
    return
  end
  env.suppress = false
  local context = env.engine.context
  env.update_connection = context.update_notifier:connect(function(updated)
    arm(env, updated)
  end)
  env.unhandled_connection = context.unhandled_key_notifier:connect(function(updated)
    arm(env, updated)
  end)
  env.option_connection = context.option_update_notifier:connect(function(updated, option)
    if option ~= "ascii_mode" then
      return
    end
    if updated:get_option("ascii_mode") then
      clear_marker(env, updated)
    else
      clear_marker(env, updated)
      arm(env, updated)
    end
  end)
end

function M.fini(env)
  if env.update_connection then
    env.update_connection:disconnect()
  end
  if env.unhandled_connection then
    env.unhandled_connection:disconnect()
  end
  if env.option_connection then
    env.option_connection:disconnect()
  end
end

return M
