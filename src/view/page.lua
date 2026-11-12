-- The page of linearpen: the issues of Linear, read, in the pane. The program asks Linear and keeps
-- the model (the groups, the filter); the page draws the lines in view, keeps the cursor and the
-- selection and tells the program what it looks at. It talks to the program only by the calls and the
-- topic of docs/calls.md, and draws with the kit's library (`@pencere/ui`).

local markdown = require("@pencere/markdown")
local kit = require("./lib/kit")
local change = require("./parts/change")
local commands = require("./parts/commands")
local context = require("./parts/context")
local data = require("./parts/data")
local head = require("./parts/head")
local help = require("./parts/help")
local list = require("./parts/list")
local lists = require("./parts/lists")
local new = require("./parts/new")
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
	return palette.is_open() or help.is_open() or lists.is_open()
end

-- While a form (the one for an agent, the one for a new issue) is open, or the title of the issue is
-- being written, its field has the keyboard, and a command that is a plain key (a letter, a digit, an
-- arrow) is not done: the key is the field's. ⌘K and ⌘Enter are not letters.
local function form_open()
	return work.is_open() or new.is_open() or panel.editing()
end
commands.when.list = function()
	return S.screen == "list" and not modal() and not form_open()
end
-- The card of keys is not offered where words are written in a form (`?` is a letter of them): the form
-- of a new issue. The form of Work on has none.
commands.when.helpable = function()
	return S.screen == "list" and not modal() and not new.is_open() and not panel.editing()
end
commands.when.keyed = function()
	return (S.screen == "list" or S.screen == "failed") and not modal() and not form_open()
end
commands.when.chord = function()
	return S.screen == "list" and not modal()
end
commands.when.idle = function()
	return commands.when.list() and not head.typing()
end
-- The list is on the page: what selects its lines, folds its groups, filters it or chooses which list
-- it is. (Where the issue has the whole pane it is not.)
commands.when.shown = function()
	return commands.when.list() and not S.alone
end
commands.when.shown_idle = function()
	return commands.when.idle() and not S.alone
end
-- What the head does (the list that is shown, the filter): the head is on the page where the issue has
-- the whole pane too, and so are its keys.
commands.when.headed = function()
	return commands.when.list()
end
-- The one issue the copies and Open in Linear are of: the issue read, the one selected or the one under
-- the cursor, where none of several is. (The copies are chords and a field does not take them.)
commands.when.single = function()
	return commands.when.list() and context.single() ~= nil
end
commands.when.single_idle = function()
	return commands.when.idle() and context.single() ~= nil
end
-- Space scrolls the open issue, but where a button has the keyboard it presses the button.
commands.when.reading = function()
	local target = u:target()
	return commands.when.idle() and panel.is_open() and not (target ~= nil and kit.buttons[target] ~= nil)
end
-- The keys that change the issues the cursor is on, or those selected: there is one to change, and no
-- change of it is on its way.
commands.when.changeable = function()
	return commands.when.idle() and change.can()
end
-- The priority and the due date need no choices of the team.
commands.when.plain_change = function()
	return commands.when.idle() and change.can(true)
end
-- The cycle and the estimate need the team to have them: not for the issues of one that has none.
commands.when.cyclable = function()
	return commands.when.changeable() and change.offers("cycle", change.targets())
end
commands.when.estimable = function()
	return commands.when.changeable() and change.offers("estimate", change.targets())
end
-- The title is of one issue, and needs none either.
commands.when.titleable = function()
	return commands.when.plain_change() and #change.targets() == 1
end
-- Work on is the open issue's: where an issue is open (its detail has the button).
commands.when.workable = function()
	return commands.when.idle() and work.workable()
end
commands.when.startable = function()
	return S.screen == "list" and not modal() and work.startable()
end
commands.when.creatable = function()
	return S.screen == "list" and not modal() and new.creatable()
end

local cmds = lib.commands(commands.for_library())

-- An issue opens beside the list, unless a form is open: that one is not put away by a press on a
-- line.
act.open_issue = function(item)
	if form_open() then
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

-- What stands beside the list: the form for a new issue, the form that hands issues to an agent, or
-- the issue.
local function side(width)
	if new.is_open() then
		return new.view(width)
	end
	if work.is_open() then
		return work.view(width)
	end
	return panel.view(width)
end

-- The head, the whole width of the page, and under it the list and, beside it (or, in a pane too narrow
-- for both, over it) the issue or a form.
local function listing()
	local over = S.panel_open and S.width < 760
	local page = b.div():col():grow(1):min_h(0)
	page:child(head.view())
	if over then
		return page:child(b.div():row():grow(1):min_h(0):child(side(S.width)))
	end
	local left = b.div():col():grow(1):min_w(0):min_h(0)
	local banner = problem()
	if banner ~= nil then
		left:child(banner)
	end
	local strip = change.strip()
	if strip ~= nil then
		left:child(strip)
	end
	left:child(body())
	local row = b.div():row():grow(1):min_h(0):child(left)
	if S.panel_open then
		row:child(side(panel.width(S.width)))
	end
	return page:child(row)
end

local function render(size, now, reduce_motion)
	u:begin(size, now, reduce_motion)
	S.now, S.width, S.height = now, size.w, size.h
	S.screen = screen()
	S.work_open = S.screen == "list" and work.is_open()
	S.new_open = S.screen == "list" and new.is_open()
	S.form_open = S.work_open or S.new_open
	S.editing = S.screen == "list" and change.editing()
	S.panel_open = S.screen == "list" and (panel.is_open() or S.form_open)
	S.room = (S.panel_open and S.width >= 760) and S.width - panel.width(S.width) or S.width
	-- The issue has the whole pane (it is too narrow for the list beside it): the list and its head
	-- are not on the page, and no key is theirs.
	S.alone = S.screen == "list" and panel.is_open() and not S.form_open and S.width < 760
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
		lists.view(root)
		help.view(root)
	end
	kit.lib.toasts_prune(S.toasts, u.now)
	if #S.toasts > 0 then
		root:child(u:toasts("toasts", { items = S.toasts }))
	end
	commands.settle()
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
	elseif id == "status" then
		return change.run("state")
	elseif id == "priority" then
		return change.run("priority")
	elseif id == "assignee" then
		return change.run("assignee")
	elseif id == "labels" then
		return change.run("labels")
	elseif id == "project" then
		return change.run("project")
	elseif id == "cycle" then
		return change.run("cycle")
	elseif id == "estimate" then
		return change.run("estimate")
	elseif id == "due" then
		return change.run("due")
	elseif id == "assign_me" then
		local ids = change.targets()
		return change.assign_me(ids)
	elseif id == "title" then
		return panel.rename_target()
	elseif id == "copy_id" then
		return context.copy("id", context.single())
	elseif id == "copy_branch" then
		return context.copy("branch", context.single())
	elseif id == "copy_link" then
		return context.copy("link", context.single())
	elseif id == "link" then
		return context.link(context.single())
	elseif id == "lists" then
		lists.show()
	elseif id == "read_down" then
		panel.scroll(1)
	elseif id == "read_up" then
		panel.scroll(-1)
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
	elseif id == "new" then
		return new.show()
	elseif id == "create" then
		return new.create()
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
	elseif id == "lists" then
		lists.show()
	elseif id == "work" then
		return work.show()
	elseif id == "new" then
		return new.show()
	elseif id == "open-linear" then
		return panel.link()
	elseif id == "panel-close" then
		panel.close()
	elseif id == "panel-title-edit" then
		return panel.rename()
	elseif string.sub(id, 1, 5) == "near:" then
		return panel.open_near(string.sub(id, 6))
	elseif id == "issue-retry" then
		panel.retry()
	elseif id == "clear-selection" then
		list.clear_marks()
	elseif id == "help-close" then
		help.hide()
	else
		local reply = status.press(id)
		if reply == nil then
			reply = work.press(id)
		end
		if reply == nil then
			reply = new.press(id)
		end
		if reply == nil then
			reply = change.press(id)
		end
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
		reply = lists.action(action)
	end
	if reply == nil then
		reply = help.action(action)
	end
	if reply == nil then
		reply = context.action(action)
	end
	if reply == nil then
		reply = work.action(action)
	end
	if reply == nil then
		reply = new.action(action)
	end
	if reply == nil then
		reply = change.action(action)
	end
	if reply ~= nil then
		return reply
	end
	if from == "toasts" and kind == "close" then
		for index, item in S.toasts do
			if item.key == action.key then
				table.remove(S.toasts, index)
				break
			end
		end
		return true
	elseif from == "status" then
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

-- Esc closes what is on top: a form (its draft is kept) or the issue, then the selection, then the
-- filter.
local function back()
	if new.is_open() then
		new.close()
	elseif work.is_open() then
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

-- A key where the issue has the whole pane: the arrows scroll what is read (the list they would
-- move in is not on the page), Page Up and Page Down by what the room shows, Home and End to its
-- ends; `j` and `k` go to the issue after and the one before. Shift and an arrow, which selects
-- lines, and Enter, which opens the one that is open, do nothing.
local LINES = 63
local function reading_key(event)
	local mods = event.mods or {}
	if mods.command or mods.control or mods.alt then
		return nil
	end
	local name = event.key
	if name == "up" or name == "down" then
		if not mods.shift then
			panel.scroll_by(name == "down" and LINES or -LINES)
		end
	elseif name == "page_down" or name == "page_up" then
		panel.scroll(name == "page_down" and 1 or -1)
	elseif name == "home" or name == "end" then
		panel.scroll_by(nil, name == "home" and "top" or "end")
	elseif (name == "j" or name == "k") and not mods.shift then
		list.step_issue(name == "j" and 1 or -1)
	elseif name ~= "enter" then
		return nil
	end
	return true
end

-- A key no component took.
local function key(event)
	local mods = event.mods or {}
	local plain = not (mods.command or mods.control or mods.alt or mods.shift)
	local target = u:target()
	if
		plain
		and (event.key == "enter" or event.key == " ")
		and target ~= nil
		and kit.buttons[target]
		-- (A button that is not drawn any more, such as the one the person pressed and that went with its
		-- screen, is not pressed by a key.)
		and b.layout.bounds(target) ~= nil
	then
		return press(target)
	end
	if S.screen == "list" and help.is_open() then
		return help.key(event)
	end
	if S.screen ~= "list" or modal() then
		return nil
	end
	if new.is_open() then
		return new.key(event)
	end
	if work.is_open() then
		-- The form takes Tab and Esc whoever holds the keyboard, and the list's keys are not its own.
		return work.key(event)
	end
	if panel.editing() then
		-- The title is being written: its keys are the field's (Enter and Esc come as its actions), and
		-- the arrows do not take the cursor from the issue.
		return nil
	end
	if u.holder ~= "page" then
		-- In the field the arrows are the list's and the words stay in it.
		return list.key(event, true) or nil
	end
	if event.key == "escape" then
		return back()
	end
	if S.alone then
		return reading_key(event)
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
	change.sync()
	if not panel.is_open() then
		return
	end
	local item = list.current()
	-- (The issue the person changed stays open, though the change took it out of the list and the
	-- cursor with it, until the cursor is moved.)
	if item ~= nil and item.kind == "issue" and item.row.id ~= panel.id() and not panel.held() then
		panel.show(item)
	end
end

-- A press of the pointer does not leave the keyboard on the control it pressed: after a click on
-- Refresh, on a part of the scope, on Close or on a value of the issue open, the arrows and Enter are
-- still the list's (Tab is what takes the keyboard to a button). A form, a card and a field (the filter,
-- the title) keep the keyboard they have, and so does a press whose reply sends the keyboard into a
-- field. (The library's own controls put the keyboard on what was pressed, a part of the scope for
-- one, whose arrows would then choose the scope: that is what is taken back.) The reply that gives it
-- back, or nil.
local function keyboard_back(reply)
	if S.screen ~= "list" or modal() or form_open() or u.holder ~= "page" or u:target() == nil then
		return nil
	end
	if type(reply) == "table" and type(reply.keyboard) == "table" then
		return nil
	end
	return u:focus(nil)
end

-- The keyboard on a control of the list's screen is there to press it and for nothing else: Tab
-- brings it, Enter or Space presses. Any other key, and any command, is the list's and takes the
-- keyboard back first, so that the arrows of a part of the scope never choose the scope and Enter
-- after them opens the issue. Esc gives it back and does no more. The reply, and whether the key ends
-- there.
local STAYS = { tab = true, enter = true, [" "] = true, space = true }
local MODS = { shift = true, control = true, alt = true, command = true, meta = true, caps_lock = true }
local function keyboard_from_control(event)
	local kind = event.type
	if kind ~= "key" and kind ~= "command" then
		return nil, false
	end
	if S.screen ~= "list" or modal() or form_open() or u.holder ~= "page" or u:target() == nil then
		return nil, false
	end
	if kind == "key" and (STAYS[event.key] or MODS[event.key]) then
		return nil, false
	end
	local mods = event.mods or {}
	local plain = not (mods.command or mods.control or mods.alt or mods.shift)
	return u:focus(nil), kind == "key" and event.key == "escape" and plain
end

local function handle(event)
	local kind = event.type
	local freed, ends = keyboard_from_control(event)
	if ends then
		return kit.take(freed)
	end
	if kind == "key" and S.screen == "list" and (new.is_open() or work.is_open()) and not modal() then
		local stepped
		if new.is_open() then
			stepped = new.tab(event)
		else
			stepped = work.tab(event)
		end
		if stepped ~= nil then
			return kit.take(stepped)
		end
	end
	local reply, actions, taken = u:handle(event)
	reply = b.merge(freed, reply)
	if kind == "editor_ready" then
		reply = b.merge(reply, new.ready(event))
	end
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
		if not taken and S.screen == "list" and not modal() then
			if (event.button or "left") == "right" then
				reply = b.merge(reply, context.press(event))
			elseif list.press(event) then
				reply = b.merge(reply, true)
			end
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
			and kit.buttons[armed]
			and (event.button or "left") == "left"
		then
			reply = b.merge(reply, press(armed))
		elseif not taken then
			reply = b.merge(reply, markdown.handle(event))
		end
		reply = b.merge(reply, keyboard_back(reply))
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
