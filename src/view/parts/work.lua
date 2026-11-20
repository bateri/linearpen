-- Handing an issue to an agent: the issue's own, from its detail (Work on is one issue's and no
-- other's; what is selected is for changing values). Beside the list (or over it, in a narrow pane) a
-- form with the issue, which agent, where it works and where its pane goes, and, where the person
-- writes some, words of theirs that end the agent's prompt. The program does the work (`work_options`,
-- `start`: the file of the issue, the prompt, the pane of bateri); the page asks it what can be chosen
-- and what the person chose.

local kit = require("../lib/kit")
local change = require("./change")
local commands = require("./commands")
local list = require("./list")
local panel = require("./panel")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local work = {}

-- The form: whether it is open; the issue (by Linear's id) and what is said of it where the list does
-- not hold it (it was open and left the list); what the program said can be chosen; what was chosen; the
-- words the person wrote for the agent; the words of why the last start did not.
local W = {
	open = false,
	serial = 0,
	id = nil,
	row = nil,
	asked = nil,
	options = nil,
	agent = nil,
	folder = "here",
	place = "split",
	-- Whether the issue, if it has not begun, is moved to its team's state of work in progress when the
	-- agent starts ("progress"), or left as it is ("keep").
	status = "progress",
	-- Whether what the last start chose (which the program kept, and `boot` brought) has been put in
	-- the form: once for a run of the page, so that what is chosen after is not undone.
	remembered = false,
	-- What the person wrote for the agent, kept while the form is put away (Esc, another issue opened
	-- in its place) for the issue it was written for (`words_of`), and let go once it is started:
	-- `restoring` while the editor made for the form has not been given them yet.
	words = "",
	words_of = nil,
	restoring = false,
	starting = false,
	error = nil,
}
work.state = W

local AGENT, FOLDER, PLACE, STATUS = "work-agent", "work-folder", "work-place", "work-status"

-- The most characters of what the person writes for the agent: the program says no to more.
local MOST = 4000

-- The editor of the words for the agent: another for each form, so that a form begins with what was
-- kept for its issue and nothing else.
local function prompt_name()
	return "work-prompt" .. W.serial
end

local function trimmed(words)
	return string.match(words, "^%s*(.-)%s*$") or ""
end

-- The words for the agent: what the editor holds (what was kept, where it has not been made yet or has
-- not been given what was kept).
local function prompt_words()
	if W.restoring then
		return W.words
	end
	local name = prompt_name()
	local length = b.layout.editor_length(name)
	if length == nil then
		return W.words
	end
	return b.layout.editor_text(name, 1, length + 1) or ""
end

-- Whether the keyboard is in the editor of the words for the agent (where the arrows move the caret and
-- choose nothing).
function work.typing()
	return W.open and u:keyboard() == u:field(prompt_name())
end

-- Whether the words for the agent are few enough to be sent.
local function prompt_fits()
	local length = utf8.len(trimmed(prompt_words()))
	return length ~= nil and length <= MOST
end

function work.is_open()
	return W.open
end

-- The issue the form is for, by Linear's id: a list of one, or none.
function work.ids()
	return W.id ~= nil and { W.id } or {}
end

-- What the program said can be chosen, once it has.
local function chosen()
	return W.options ~= nil and W.options.status == "ready" and W.options.data or nil
end

-- The kinds of state that have not begun: the issues in them are what Status moves.
local NOT_BEGUN = { triage = true, backlog = true, unstarted = true }

-- The first state of work in progress of a team's choices (they come in the order of their kinds, and
-- by position within one), or nil.
local function first_started(data)
	for _, state in data.states do
		if state.kind == "started" then
			return state
		end
	end
	return nil
end

-- What Status would do for the issue of the form: whether it has not begun and is of a team whose
-- state of work in progress is known (`count` 1), and what that state is called. nil where it would not
-- move. The program decides again, by what Linear says then; this is only what the form can promise.
function work.moving()
	local count, name, mixed = 0, nil, false
	for _, id in work.ids() do
		local item = list.item(id)
		local team, kind
		if item ~= nil then
			team, kind = item.row.teamId, item.group.kind
		elseif W.row ~= nil then
			team, kind = W.row.teamId, W.row.kind
		end
		local data = team ~= nil and NOT_BEGUN[kind] and change.options(team) or nil
		local state = data ~= nil and first_started(data) or nil
		if state ~= nil then
			count += 1
			if name ~= nil and name ~= state.name then
				mixed = true
			end
			name = name or state.name
		end
	end
	if count == 0 then
		return nil
	end
	return { count = count, name = mixed and "Started" or name }
end

-- Whether the issue can be handed over now: one is open (its detail is where the button is), and no
-- form is.
function work.workable()
	return not W.open and panel.is_open()
end

-- Whether what Status would do is known: the choices of the team of the issue, if it has not begun, are
-- here (or could not come, and then the row is not there and nothing is moved). Until they are, Start
-- waits, so that what the form promises (the row, and the choice kept for it) is what is sent.
local function status_known()
	for _, id in work.ids() do
		local item = list.item(id)
		local team, kind
		if item ~= nil then
			team, kind = item.row.teamId, item.group.kind
		elseif W.row ~= nil then
			team, kind = W.row.teamId, W.row.kind
		end
		if team ~= nil and NOT_BEGUN[kind] and not change.ready(team) then
			return false
		end
	end
	return true
end

-- Whether the agent can be started now.
function work.startable()
	local options = chosen()
	return W.open
		and not W.starting
		and status_known()
		and options ~= nil
		and options.unavailable == nil
		and W.agent ~= nil
		and W.id ~= nil
		and prompt_fits()
end

-- Asks the program what can be chosen (it is the same whichever the issue is), once for a form. The
-- keyboard goes to the choice of the agent when it comes, so that the arrows choose.
local function ask(ids)
	if ids[1] == nil or W.asked then
		return
	end
	W.asked = true
	W.options = { status = "loading" }
	local serial = W.serial
	kit.call("work_options", {}, function(result, err)
		if not W.asked then
			return true
		end
		if err then
			W.options = { status = "failed", error = err.message or err.code }
			return true
		end
		W.options = { status = "ready", data = result }
		local known = false
		for _, agent in result.agents do
			known = known or agent.id == W.agent
		end
		if not known then
			W.agent = result.agents[1] ~= nil and result.agents[1].id or nil
		end
		if W.serial == serial and W.open and W.agent ~= nil and u.holder == "page" then
			return u:focus(u:name(AGENT, "item", W.agent))
		end
		return true
	end)
end

-- Keeps what is asked of the program to the issue the form is for.
function work.sync()
	if W.open then
		ask(work.ids())
		-- What Status can move the issue to is what its team says can be chosen.
		for _, id in work.ids() do
			local item = list.item(id)
			local team = item ~= nil and item.row.teamId or (W.row ~= nil and W.row.teamId or nil)
			change.ask(team, false)
		end
	end
end

-- Puts what the last start chose into the form, the first time it is opened: the agent, where the
-- agent works, where its pane goes and Status. What the program says is not there (an agent that is
-- not installed) is let go when it says what can be chosen (`ask`). The words for the agent are not
-- among them: they are for one issue.
local function remember()
	if W.remembered then
		return
	end
	W.remembered = true
	local prefs = S.boot ~= nil and S.boot.prefs or nil
	if prefs == nil then
		return
	end
	if prefs.agent ~= nil then
		W.agent = prefs.agent
	end
	if prefs.folder == "here" or prefs.folder == "worktree" then
		W.folder = prefs.folder
	end
	if prefs.place == "split" or prefs.place == "tab" then
		W.place = prefs.place
	end
	if prefs.status == "progress" or prefs.status == "keep" then
		W.status = prefs.status
	end
end

-- Opens the form for the issue that is open. The reply.
function work.show()
	if not work.workable() then
		return true
	end
	remember()
	local info = panel.info()
	W.row = {
		identifier = info.identifier,
		title = info.title,
		teamId = info.teamId,
		kind = info.state ~= nil and info.state.kind or nil,
	}
	panel.close()
	W.open, W.id = true, info.id
	-- The words kept are of the issue they were written for; another issue's form begins empty.
	if W.words_of ~= info.id then
		W.words, W.words_of = "", info.id
	end
	W.restoring = W.words ~= ""
	W.serial += 1
	-- (A start that is on its way is not forgotten: its answer comes to a form that may be gone.)
	W.asked, W.options, W.error = nil, nil, nil
	work.sync()
	return u:focus(nil)
end

-- Puts the form away and keeps the words for the agent.
function work.close()
	if not W.open then
		return
	end
	W.words = prompt_words()
	W.restoring = false
	W.open = false
end

-- The words kept are put back in the editor the host has made, with the caret after them. The reply,
-- or nil.
function work.ready(event)
	if not W.restoring or event.editor ~= prompt_name() then
		return nil
	end
	W.restoring = false
	if W.words == "" then
		return nil
	end
	return { editor = { [prompt_name()] = { { load = W.words }, { command = "move", movement = "document_end" } } } }
end

-- The editor of the words changed: a frame is drawn, for how many there are says whether Start can be
-- pressed. The reply, or nil.
function work.edited(event)
	if W.open and event.editor == prompt_name() then
		return true
	end
	return nil
end

-- Hands the issue over to the program.
function work.start()
	if not work.startable() then
		return true
	end
	W.starting, W.error = true, nil
	-- Status goes only where the form had the row, and then as it said: to move (true) or to leave
	-- (false). Without the row there is nothing to move and nothing to remember.
	local progress = nil
	if work.moving() ~= nil then
		progress = W.status == "progress"
	end
	local serial, id = W.serial, W.id
	local words = trimmed(prompt_words())
	kit.call("start", {
		id = W.id,
		agent = W.agent,
		folder = W.folder,
		open = W.place,
		progress = progress,
		prompt = words ~= "" and words or nil,
	}, function(result, err)
		W.starting = false
		-- The words went with a start that was made: where the form was put away while it went (and
		-- they were kept), the next form of the issue does not have them.
		if not err and not W.open and W.words_of == id then
			W.words, W.words_of = "", nil
		end
		-- The form it was sent from may have been put away, and another opened: that one is not done.
		if W.serial ~= serial or not W.open then
			return true
		end
		if err then
			W.error = err.message or err.code
			return true
		end
		-- It is on its way: the form is done. Where the issue was to move and Linear did not, the list's
		-- strip says so (the agent is started all the same).
		if result ~= nil and result.notMoved ~= nil then
			change.note(result.notMoved)
		end
		-- The words went with it: the next form of the issue begins empty.
		W.open, W.restoring = false, false
		W.words, W.words_of = "", nil
		return u:focus(nil)
	end)
	return true
end

-- --- The form ------------------------------------------------------------------------------------

local LABEL = 66

-- A row of the form: what is chosen and the control that chooses it.
local function field(label, control)
	return b.div():row():items_center():gap(12):no_shrink():child(
		b.div():w(LABEL):no_shrink():child(b.text(label):color("dim"):text_size(SIZE.small))
	):child(control)
end

local function agents_control(options)
	local items = {}
	for _, agent in options.agents do
		items[#items + 1] = { key = agent.id, title = agent.name }
	end
	if #items == 0 then
		return b.text("None installed"):color("dim"):text_size(SIZE.small)
	end
	return u:segments(AGENT, { value = W.agent, items = items })
end

local function folder_control(options)
	return u:segments(FOLDER, {
		value = W.folder,
		items = {
			{ key = "here", title = "This folder", tip = "Work in " .. (options.folder.name or "the folder linearpen began in") },
			{
				key = "worktree",
				title = "New worktree",
				tip = "The agent makes a worktree first, by this project's own rules",
			},
		},
	})
end

-- The words for the agent: an editor the host keeps, that wraps, in a box of the field's look. A line
-- in a short pane, for Start to stay on the page.
local function prompt_box(compact)
	local t = u.theme
	local name = u:field(prompt_name())
	local ringed = u:keyboard() == name
	local editor = b.editor(name, {
		wrap = true,
		gutter = false,
		padding = 8,
		line_height = 1.5,
		hint = "More for the agent, such as: release it when it is done",
		read_only = W.starting,
		-- Tab is the page's: it goes on to Start.
		keymap = { tab = false, ["shift+tab"] = false },
	}):grow(1):min_h(0):label("Prompt")
	return b.div()
		:h(compact and 38 or 76)
		:grow(1)
		:min_w(0)
		:rounded(t.radius.control)
		:bg(t.field)
		:border(1, ringed and t.ring or t.line)
		:motion({ seconds = t.motion, color = true })
		:child(editor)
end

local function place_control()
	return u:segments(PLACE, {
		value = W.place,
		items = {
			{ key = "split", title = "Split right", tip = "Open the agent in a pane to the right of this one" },
			{ key = "tab", title = "New tab", tip = "Open the agent in a tab of its own" },
		},
	})
end

-- Whether the issues that have not begun are moved: to what their team calls its state of work in
-- progress, or not.
local function status_control(moving)
	return u:segments(STATUS, {
		value = W.status,
		items = {
			{
				key = "progress",
				title = moving.name,
				tip = "Move the issues that have not begun to " .. moving.name .. " when the agent starts",
			},
			{ key = "keep", title = "Unchanged", tip = "Leave the status of the issues as it is" },
		},
	})
end

local function start_label()
	local options = chosen()
	if options ~= nil then
		for _, agent in options.agents do
			if agent.id == W.agent then
				return "Start " .. agent.name
			end
		end
	end
	return "Start"
end

-- The button that starts the agent, and why it cannot (words), where it cannot.
local function start_button()
	local options = chosen()
	local why = nil
	if options ~= nil and options.unavailable ~= nil then
		why = options.unavailable
	elseif not prompt_fits() then
		why = "A prompt is up to " .. MOST .. " characters"
	end
	local button = kit.button("work-start", start_label(), {
		primary = true,
		suffix = commands.label("start"),
		busy = W.starting,
		disabled = not work.startable() or W.starting,
		tip = why or "Start the agent in a new pane",
	})
	return button, why
end

function work.view(width)
	local compact = S.height < SIZE.short
	local options = chosen()
	local row = list.issue(W.id) or W.row or {}
	local top = b.div():col():no_shrink():pt(SIZE.head_top):px(SIZE.pad):pb(14)
	top:child(
		b.div():row():items_center():gap(8):no_shrink():child(
			b.text("Work on " .. (row.identifier or "")):text_size(15):weight(700)
		):child(b.spacer()):child(kit.button("work-close", "Close", { suffix = "Esc" }))
	)
	top:child(b.text(row.title or ""):wrap():mt(6):no_shrink())
	local content = b.div():col():grow(1):min_w(0):min_h(0):pt(14):px(SIZE.pad):gap(compact and 8 or 12)
	if W.options == nil or W.options.status == "loading" then
		content:child(b.div():row():items_center():gap(8):no_shrink():child(kit.spinner(14, "quiet")):child(
			b.text("Looking at what can be chosen"):color("dim"):text_size(SIZE.small)
		))
	elseif options == nil then
		content:child(
			b.text(W.options.error or "The program did not say what can be chosen"):color("warning"):wrap():text_size(SIZE.small)
		)
	else
		content:child(field("Agent", agents_control(options)))
		-- (No line under Folder: choosing moves nothing below it, and the choice's tip says who makes
		-- the worktree.)
		content:child(field("Folder", folder_control(options)))
		content:child(field("Open in", place_control()))
		local moving = work.moving()
		if moving ~= nil then
			content:child(field("Status", status_control(moving)))
		end
		-- (The label stands by the first line of the box, not at its middle.)
		content:child(b.div():row():items_start():gap(12):no_shrink():child(
			b.div():w(LABEL):no_shrink():mt(10):child(b.text("Prompt"):color("dim"):text_size(SIZE.small))
		):child(prompt_box(compact)))
	end
	local button, why = start_button()
	local foot = b.div():row():items_center():gap(12):no_shrink():mt(compact and 4 or 8):child(button)
	if why ~= nil then
		foot:child(b.text(why):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1))
	end
	content:child(foot)
	if W.error ~= nil then
		content:child(b.text(W.error):text_size(SIZE.small):color("warning"):wrap())
	end
	local box = b.div():col():grow(1):min_w(0):min_h(0):child(top)
	box:child(b.div():h(1):bg(b.alpha("foreground", 0.18)):no_shrink())
	box:child(b.div():id("work-scroll"):scroll():col():grow(1):min_w(0):min_h(0):pb(SIZE.pad):child(content))
	local side = b.div():row():w(width):no_shrink():min_h(0)
	if width < S.width then
		side:child(b.div():w(1):bg(b.alpha("foreground", 0.23)):no_shrink())
	end
	return side:child(box)
end

-- --- What the person does ------------------------------------------------------------------------

-- A press on one of the form's own buttons or boxes. The reply, or nil where it was not the form's.
function work.press(id)
	if id == "work-close" then
		work.close()
		return u:focus(nil)
	elseif id == "work-start" then
		return work.start()
	end
	return nil
end

-- What a control of the form chose. The reply, or nil where it was not the form's.
function work.action(action)
	if action.kind ~= "select" then
		return nil
	end
	if action.from == AGENT then
		W.agent = action.key
	elseif action.from == FOLDER then
		W.folder = action.key
	elseif action.from == PLACE then
		W.place = action.key
	elseif action.from == STATUS then
		W.status = action.key
	else
		return nil
	end
	return true
end

-- What Tab goes through, in order, as it stands now: the choice made in each control (the one of its
-- parts that is in the order of the keyboard), the words for the agent, and the button that starts.
local function order()
	local names = {}
	local controls = { { AGENT, W.agent }, { FOLDER, W.folder }, { PLACE, W.place }, { STATUS, W.status } }
	for _, control in controls do
		local name = control[2] ~= nil and u:name(control[1], "item", control[2]) or nil
		if name ~= nil and b.layout.bounds(name) ~= nil then
			names[#names + 1] = name
		end
	end
	if W.options ~= nil and W.options.status == "ready" then
		names[#names + 1] = u:field(prompt_name())
	end
	if b.layout.bounds("work-start") ~= nil then
		names[#names + 1] = "work-start"
	end
	return names
end

-- Tab (and Shift Tab) goes on to the next field of the form, and round: the library goes through the
-- whole page, and the form is a place of its own. The reply, or nil where the key is not Tab.
function work.tab(event)
	local mods = event.mods or {}
	if event.key ~= "tab" or mods.command or mods.control or mods.alt then
		return nil
	end
	local names = order()
	local at = 0
	local now = u:target()
	for index, name in names do
		if name == now then
			at = index
		end
	end
	local step = mods.shift and -1 or 1
	return u:focus(names[((at - 1 + step) % #names) + 1])
end

-- Esc, which the form takes whoever holds the keyboard. (The arrows choose in a control that has the
-- keyboard.)
function work.key(event)
	local mods = event.mods or {}
	if event.key == "escape" and not (mods.command or mods.control or mods.alt or mods.shift) then
		work.close()
		return u:focus(nil)
	end
	return nil
end

-- --- The agent at work ---------------------------------------------------------------------------

-- The agent that works on the issue `id` (`{ agent, idle }`, as the topic's `working` says), or nil.
function work.agent_of(id)
	local working = S.issues ~= nil and S.issues.working or nil
	if id == nil or working == nil then
		return nil
	end
	return working[id]
end

-- Takes the keyboard to the pane of the agent that works on the issue `id`: the program knows the pane
-- and asks bateri. Nothing is said where it went (the keyboard is there); where it did not, why. The
-- reply.
function work.go_to(id)
	if work.agent_of(id) == nil then
		return true
	end
	kit.call("go_to_agent", { id = id }, function(_, err)
		if err then
			kit.toast(err.message or err.code or "bateri did not go to the agent's pane", "warn")
		end
		return true
	end)
	return true
end

act.agent_of = work.agent_of
act.go_to_agent = work.go_to

return work
