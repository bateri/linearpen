-- The page of linearpen: the issues of Linear, read, in the pane. The program asks Linear and keeps
-- the model (the groups, the filter); the page draws the lines in view, keeps the cursor and the
-- selection and tells the program what it looks at. It talks to the program only by the calls and the
-- topic of docs/calls.md, and draws with the kit's library (`@pencere/ui`).

local markdown = require("@pencere/markdown")
local kit = require("./lib/kit")
local commands = require("./parts/commands")
local data = require("./parts/data")
local head = require("./parts/head")
local help = require("./parts/help")
local list = require("./parts/list")
local palette = require("./parts/palette")
local panel = require("./parts/panel")
local screens = require("./parts/screens")
local status = require("./parts/status")
local work = require("./parts/work")

local b, u, S, SIZE, act, lib = kit.b, kit.u, kit.S, kit.SIZE, kit.act, kit.lib

-- --- The commands -----------------------------------------------------------------------------------

-- What a command can be done under: what the page shows, never where the keyboard is, but for the one
-- whose key a field would take (`idle`).
local function modal()
	return palette.is_open() or help.is_open()
end

-- While the form for an agent is open its fields have the keyboard, and a command that is a plain key
-- (a letter, a digit, an arrow) is not done: the key is the field's. ⌘K and ⌘Enter are not letters.
commands.when.list = function()
	return S.screen == "list" and not modal() and not work.is_open()
end
commands.when.keyed = function()
	return (S.screen == "list" or S.screen == "failed") and not modal() and not work.is_open()
end
commands.when.chord = function()
	return S.screen == "list" and not modal()
end
commands.when.idle = function()
	return commands.when.list() and not head.typing()
end
commands.when.issue = function()
	return commands.when.list() and panel.detail() ~= nil
end
commands.when.workable = function()
	return commands.when.idle() and work.workable()
end
commands.when.startable = function()
	return S.screen == "list" and not modal() and work.startable()
end

local cmds = lib.commands(commands.for_library())

-- An issue opens beside the list, unless the form that hands issues to an agent is open: that one is
-- not put away by a press on a line.
act.open_issue = function(item)
	if work.is_open() then
		return true
	end
	return panel.show(item)
end

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
	return list.view(S.room, S.panel_open)
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

-- What stands beside the list: the form that hands issues to an agent, or the issue.
local function side(width)
	if work.is_open() then
		return work.view(width)
	end
	return panel.view(width)
end

-- The list with its head, and beside it (or, in a pane too narrow for both, over it) the issue.
local function listing()
	local over = S.panel_open and S.width < 760
	if over then
		return b.div():row():grow(1):min_h(0):child(side(S.width))
	end
	local left = b.div():col():grow(1):min_w(0):min_h(0)
	left:child(head.view())
	local banner = problem()
	if banner ~= nil then
		left:child(banner)
	end
	left:child(body())
	local row = b.div():row():grow(1):min_h(0):child(left)
	if S.panel_open then
		row:child(side(panel.width(S.width)))
	end
	return row
end

local function render(size, now, reduce_motion)
	u:begin(size, now, reduce_motion)
	S.now, S.width, S.height = now, size.w, size.h
	S.screen = screen()
	S.work_open = S.screen == "list" and work.is_open()
	S.panel_open = S.screen == "list" and (panel.is_open() or work.is_open())
	S.room = (S.panel_open and S.width >= 760) and S.width - panel.width(S.width) or S.width
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
		root:child(listing())
		root:child(status.view())
		palette.view(root)
		help.view(root)
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
	elseif id == "select" then
		list.toggle_mark(list.state.cursor)
	elseif id == "select_all" then
		list.select_all()
	elseif id == "link" then
		return panel.link()
	elseif id == "scope_mine" then
		act.view({ scope = "mine" })
	elseif id == "scope_all" then
		act.view({ scope = "all" })
	elseif id == "filter" then
		return head.focus()
	elseif id == "refresh" then
		act.refresh()
	elseif id == "work" then
		return work.show()
	elseif id == "start" then
		return work.start()
	elseif id == "palette" then
		palette.show()
	elseif id == "help" then
		help.show()
	end
	return true
end

act.run = command

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
	elseif id == "work" then
		return work.show()
	elseif id == "open-linear" then
		return panel.link()
	elseif id == "panel-close" then
		panel.close()
	elseif id == "issue-retry" then
		panel.retry()
	elseif id == "clear-selection" then
		list.clear_marks()
	elseif id == "help-close" then
		help.hide()
	else
		local reply = work.press(id)
		if reply ~= nil then
			return reply
		end
	end
	return true
end

local function on_action(action)
	local from, kind = action.from, action.kind
	local reply = head.action(action)
	if reply == nil then
		reply = palette.action(action)
	end
	if reply == nil then
		reply = help.action(action)
	end
	if reply == nil then
		reply = work.action(action)
	end
	if reply ~= nil then
		return reply
	end
	if from == "status" then
		status.action(action)
		return true
	elseif from == "key" and kind == "submit" then
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

-- Esc closes what is on top: the form for an agent or the issue, then the selection, then the filter.
local function back()
	if work.is_open() then
		work.close()
	elseif panel.is_open() then
		panel.close()
	elseif list.marked() > 0 then
		list.clear_marks()
	elseif head.field.value ~= "" then
		head.set("")
	else
		return nil
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
	if S.screen ~= "list" or modal() then
		return nil
	end
	if work.is_open() then
		-- The form takes Tab and Esc whoever holds the keyboard, and the list's keys are not its own.
		return work.key(event)
	end
	if u.holder ~= "page" then
		-- In the field the arrows are the list's and the words stay in it.
		return list.key(event, true) or nil
	end
	if event.key == "escape" then
		return back()
	end
	return list.key(event, false) or nil
end

-- The pointer is over something the page answers itself (a line, a button): the hand.
local function shape(event)
	local over = b.layout.hit({ x = event.x, y = event.y })
	local wanted = nil
	if
		over ~= nil
		and (
			kit.buttons[over]
			or string.sub(over, 1, 4) == "row:"
			or string.sub(over, 1, 6) == "check:"
			or string.sub(over, 1, 11) == "work-check:"
		)
	then
		wanted = "pointer"
	end
	if wanted == S.cursor then
		return nil
	end
	S.cursor = wanted
	return { cursor = wanted or "arrow" }
end

-- The issue open follows the cursor where the cursor is on an issue.
local function follow()
	work.sync()
	if not panel.is_open() then
		return
	end
	local item = list.current()
	if item ~= nil and item.kind == "issue" and item.row.id ~= panel.id() then
		panel.show(item)
	end
end

local function handle(event)
	local kind = event.type
	if kind == "key" and S.screen == "list" and work.is_open() and not modal() then
		local stepped = work.tab(event)
		if stepped ~= nil then
			return kit.take(stepped)
		end
	end
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
		if not taken and S.screen == "list" and not modal() and list.press(event) then
			reply = b.merge(reply, true)
		end
		if not taken then
			reply = b.merge(reply, markdown.handle(event))
		end
	elseif kind == "release" then
		local armed, over = S.armed, b.layout.hit({ x = event.x, y = event.y })
		S.armed = nil
		if
			armed ~= nil
			and armed == over
			and (kit.buttons[armed] or string.sub(armed, 1, 11) == "work-check:")
			and (event.button or "left") == "left"
		then
			reply = b.merge(reply, press(armed))
		elseif not taken then
			reply = b.merge(reply, markdown.handle(event))
		end
	elseif kind == "key" and not taken then
		reply = b.merge(reply, key(event))
	end
	follow()
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
