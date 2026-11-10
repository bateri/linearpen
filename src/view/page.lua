-- The page of linearpen: the issues of Linear, read, in the pane. The program asks Linear and keeps
-- the model (the groups, the filter); the page draws the lines in view, keeps the cursor and tells
-- the program what it looks at. It talks to the program only by the calls and the topic of
-- docs/calls.md, and draws with the kit's library (`@pencere/ui`).

local kit = require("./lib/kit")
local commands = require("./parts/commands")
local data = require("./parts/data")
local head = require("./parts/head")
local list = require("./parts/list")
local screens = require("./parts/screens")
local status = require("./parts/status")

local b, u, S, SIZE, act, lib = kit.b, kit.u, kit.S, kit.SIZE, kit.act, kit.lib

-- --- The commands -----------------------------------------------------------------------------------

-- What a command can be done under: what the page shows, never where the keyboard is.
commands.when.list = function()
	return S.screen == "list"
end
commands.when.keyed = function()
	return S.screen == "list" or S.screen == "failed"
end

local cmds = lib.commands(commands.for_library())

-- --- Which screen -----------------------------------------------------------------------------------

-- What the page shows: "boot" until the program answers, "connect" where there is no key (or Linear
-- refused the one there is, before any list came), "loading" until the first list, "failed" where it
-- could not come, "list" otherwise.
local function screen()
	if S.boot == nil then
		return "boot"
	end
	local snapshot = S.issues
	local error = snapshot ~= nil and snapshot.error or nil
	if not S.has_key or (error ~= nil and error.kind == "refused" and not snapshot.loaded) then
		return "connect"
	end
	if snapshot == nil or (not snapshot.loaded and error == nil) then
		return "loading"
	end
	if not snapshot.loaded then
		return "failed"
	end
	return "list"
end

local function booting()
	local box = b.div():grow(1):col():items_center():justify_center():gap(8)
	if S.boot_error then
		box:child(b.text("The program did not answer"):weight(650))
		box:child(b.text(S.boot_error):color("dim"):wrap():max_w(520):align("center"))
	else
		box:child(kit.spinner(20, "quiet"))
	end
	return box
end

-- The list's room: the lines, or what stands where they would.
local function body()
	local snapshot = S.issues
	if list.count() == 0 then
		if S.scope == "mine" and head.field.value == "" and snapshot.counts.mine == 0 then
			return screens.empty()
		end
		return screens.nothing(head.field.value)
	end
	return list.view(S.width)
end

-- The line that says the last asking failed while the lists of before are still shown.
local function problem()
	local error = S.issues.error
	if error == nil then
		return nil
	end
	return u:banner("problem", {
		tone = "warn",
		icon = "assets/icons/alert-triangle.svg",
		words = error.message,
		actions = { { key = "retry", title = "Try again" } },
	})
end

local function render(size, now, reduce_motion)
	u:begin(size, now, reduce_motion)
	S.now, S.width, S.height = now, size.w, size.h
	S.screen = screen()
	local root = b.div():size_full():col():bg("background"):color("foreground"):family("terminal")
	if S.screen == "boot" then
		root:child(booting())
	elseif S.screen == "connect" then
		root:child(screens.connect())
		root:child(status.connect())
	elseif S.screen == "loading" then
		root:child(screens.loading())
		root:child(status.empty())
	elseif S.screen == "failed" then
		root:child(screens.failed(S.issues.error))
		root:child(status.empty())
	else
		root:child(head.view())
		local banner = problem()
		if banner ~= nil then
			root:child(banner)
		end
		root:child(body())
		root:child(status.view())
	end
	cmds:write(u, root)
	return u:finish(root)
end

-- --- What the person does ------------------------------------------------------------------------------

local function command(id)
	if id == "fold" then
		list.fold(true)
	elseif id == "unfold" then
		list.fold(false)
	elseif id == "scope_mine" then
		act.view({ scope = "mine" })
	elseif id == "scope_all" then
		act.view({ scope = "all" })
	elseif id == "filter" then
		return head.focus()
	elseif id == "refresh" then
		act.refresh()
	end
	return true
end

-- A button of the page's own pressed (kit.button).
local function press(id)
	if id == "refresh" or id == "retry" then
		act.refresh()
	elseif id == "show-all" then
		act.view({ scope = "all" })
	elseif id == "connect" then
		act.connect(screens.field.value)
	elseif id == "create-key" then
		return { link = screens.KEYS_URL }
	end
	return true
end

local function on_action(action)
	local from, kind = action.from, action.kind
	local reply = head.action(action)
	if reply ~= nil then
		return reply
	end
	if from == "key" and kind == "submit" then
		act.connect(screens.field.value)
		return true
	elseif from == "problem" and kind == "press" then
		act.refresh()
		return true
	elseif from == "commands" and kind == "command" then
		return command(action.id)
	end
	return true
end

-- A key no component took.
local function key(event)
	local mods = event.mods or {}
	local plain = not (mods.command or mods.control or mods.alt or mods.shift)
	local target = u:target()
	if plain and (event.key == "enter" or event.key == " ") and target ~= nil and kit.buttons[target] then
		return press(target)
	end
	if S.screen ~= "list" then
		return nil
	end
	if u.holder ~= "page" then
		-- In the field the arrows are the list's and the words stay in it.
		return list.key(event, true) or nil
	end
	if event.key == "escape" and head.field.value ~= "" then
		head.set("")
		return true
	end
	return list.key(event, false) or nil
end

-- The pointer is over something the page answers itself (a line, a button): the hand.
local function shape(event)
	local over = b.layout.hit({ x = event.x, y = event.y })
	local wanted = nil
	if over ~= nil and (kit.buttons[over] or string.sub(over, 1, 4) == "row:") then
		wanted = "pointer"
	end
	if wanted == S.cursor then
		return nil
	end
	S.cursor = wanted
	return { cursor = wanted or "arrow" }
end

local function handle(event)
	local kind = event.type
	local reply, actions, taken = u:handle(event)
	for _, action in actions do
		reply = b.merge(reply, on_action(action))
	end
	if kind == "move" then
		reply = b.merge(reply, shape(event))
	elseif kind == "leave" then
		S.cursor = nil
	elseif kind == "press" then
		if (event.button or "left") == "left" then
			S.armed = b.layout.hit({ x = event.x, y = event.y })
		end
		if not taken and S.screen == "list" and list.press(event) then
			reply = b.merge(reply, true)
		end
	elseif kind == "release" then
		local armed, over = S.armed, b.layout.hit({ x = event.x, y = event.y })
		S.armed = nil
		if armed ~= nil and armed == over and kit.buttons[armed] and (event.button or "left") == "left" then
			reply = b.merge(reply, press(armed))
		end
	elseif kind == "key" and not taken then
		reply = b.merge(reply, key(event))
	end
	return kit.take(reply)
end

-- --- Starting -----------------------------------------------------------------------------------------------

data.start()

return {
	render = render,
	settle = function()
		local moved = list.settle()
		return u:settle() or moved
	end,
	handle = handle,
}
