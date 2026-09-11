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
  -- The mobile default menu is an idle-state prompt, not a keyboard-selectable
  -- composition.  In particular, mobile frontends can use modified arrows to
  -- switch to a symbol layer; leaving the marker active lets the next symbol
  -- key reach `selector` and commit the first default candidate.  Dismiss the
  -- prompt for every key press.  Touch selection is unaffected.
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
