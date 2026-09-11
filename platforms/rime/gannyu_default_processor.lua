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
  if env.suppress then
    return
  end
  if context.input ~= "" then
    env.idle_dismissed = false
    return
  end
  if not env.idle_dismissed and not context:get_option("ascii_mode") then
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

local function dismiss_marker(env, context)
  env.idle_dismissed = true
  clear_marker(env, context)
end

local M = {}

function M.func(key, env)
  local context = env.engine.context
  if context.input ~= marker or key:release() then
    return 2
  end
  -- The mobile default menu is an idle-state prompt, not a keyboard-selectable
  -- composition.  Dismiss it for every key press and keep it dismissed until
  -- a real composition starts.  Returning kNoop is required to let the
  -- frontend receive symbols, so the unhandled-key notifier cannot re-arm it.
  dismiss_marker(env, context)
  return 2
end

function M.init(env)
  if not is_mobile_rime() then
    return
  end
  env.suppress = false
  env.idle_dismissed = false
  local context = env.engine.context
  env.update_connection = context.update_notifier:connect(function(updated)
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
      env.idle_dismissed = false
      arm(env, updated)
    end
  end)
  arm(env, context)
end

function M.fini(env)
  if env.update_connection then
    env.update_connection:disconnect()
  end
  if env.option_connection then
    env.option_connection:disconnect()
  end
end

return M
