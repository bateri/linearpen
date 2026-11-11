-- Handing issues to an agent: beside the list (or over it, in a narrow pane) a form with the issues,
-- what to tell the agent, which agent, where it works and where its pane goes. The program does the
-- work (`work_options`, `start`: the file of issues, the worktree, the pane of bateri); the page
-- asks it what can be chosen and what the person chose.

local kit = require("../lib/kit")
local change = require("./change")
local commands = require("./commands")
local list = require("./list")
local panel = require("./panel")
local row = require("./row")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local work = {}

-- The form: whether it is open; the issues (those selected, as they are selected, or the one the
-- cursor was on); what the program said can be chosen; what was chosen; the words of why the last
-- start did not.
local W = {
	open = false,
	serial = 0,
	live = false,
	fixed = {},
	-- What is said of an issue the list does not hold (one that is open and left the list): by id.
	rows = {},
	asked = nil,
	options = nil,
	agent = nil,
	folder = "here",
	place = "split",
	-- Whether the issues that have not begun are moved to their team's state of work in progress
	-- when the agent starts ("progress"), or left as they are ("keep").
	status = "progress",
	-- Whether what the last start chose (which the program kept, and `boot` brought) has been put in
	-- the form: once for a run of the page, so that what is chosen after is not undone.
	remembered = false,
	starting = false,
	error = nil,
}
work.state = W

local AGENT, FOLDER, PLACE, STATUS = "work-agent", "work-folder", "work-place", "work-status"

-- The most issues one agent is started on (the program's limit, docs/calls.md).
work.MOST = 25

-- The editor of the words for the agent: another each time the form opens, so that it begins empty.
local function prompt_name()
	return "work-prompt" .. W.serial
end

function work.is_open()
	return W.open
end

-- The issues the form is for, by Linear's id.
function work.ids()
	if W.live then
		return list.marked_ids()
	end
	return W.fixed
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

-- What Status would do for the issues of the form: how many have not begun and are of a team whose
-- state of work in progress is known, and what that state is called (one name, or "Started" where the
-- teams call it differently). nil where none would move. The program decides again, by what Linear
-- says then; this is only what the form can promise.
function work.moving()
	local count, name, mixed = 0, nil, false
	for _, id in work.ids() do
		local item = list.item(id)
		local team, kind
		if item ~= nil then
			team, kind = item.row.teamId, item.group.kind
		elseif W.rows[id] ~= nil then
			team, kind = W.rows[id].teamId, W.rows[id].kind
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

-- What Work on is for, which is also where its button stands: "marks", the issues selected where
-- they are seen (the button is by their count, in the head); "panel", the issue that is open (in
-- its own panel); "cursor", the issue under the cursor (at the end of the head, and named). Where
-- the issue has the whole pane what is selected is not on the page, and it is the issue.
function work.target()
	if list.marked() > 0 and not S.alone then
		return "marks"
	end
	if panel.is_open() then
		return "panel"
	end
	local item = list.current()
	if item ~= nil and item.kind == "issue" then
		return "cursor"
	end
	return nil
end

-- Whether an issue can be handed over now.
function work.workable()
	return not W.open and work.target() ~= nil
end

-- For the head and the panel, which draw the button: what it is for, or nil where there is none
-- (a form is open in its place).
act.work_target = function()
	if W.open or S.form_open then
		return nil
	end
	return work.target()
end

-- Whether what Status would do is known: the choices of the team of each issue that has not begun are
-- here (or could not come, and then the row is not there and nothing is moved). Until they are, Start
-- waits, so that what the form promises (the row, and the choice kept for it) is what is sent.
local function status_known()
	for _, id in work.ids() do
		local item = list.item(id)
		local team, kind
		if item ~= nil then
			team, kind = item.row.teamId, item.group.kind
		elseif W.rows[id] ~= nil then
			team, kind = W.rows[id].teamId, W.rows[id].kind
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
		and #work.ids() > 0
		and #work.ids() <= work.MOST
end

-- Asks the program what can be chosen (it is the same whichever the issues are), once for a form.
local function ask(ids)
	if ids[1] == nil or W.asked then
		return
	end
	W.asked = true
	W.options = { status = "loading" }
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
		if W.folder == "worktree" and not result.worktree.available then
			W.folder = "here"
		end
		return true
	end)
end

-- Keeps what is asked of the program to the issues the form is for.
function work.sync()
	if W.open then
		ask(work.ids())
		-- What Status can move the issues to is what their teams say can be chosen.
		for _, id in work.ids() do
			local item = list.item(id)
			local team = item ~= nil and item.row.teamId or (W.rows[id] ~= nil and W.rows[id].teamId or nil)
			change.ask(team, false)
		end
	end
end

-- Puts what the last start chose into the form, the first time it is opened: the agent, where the
-- agent works, where its pane goes and Status. What the program says is not there (an agent that is
-- not installed, a worktree outside a repository) is let go when it says what can be chosen (`ask`).
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

-- Opens the form for the issues selected, or the one the cursor is on. The reply that gives the
-- keyboard to the words for the agent.
function work.show()
	local target = work.target()
	if W.open or target == nil then
		return true
	end
	remember()
	local live = target == "marks"
	local ids, rows = {}, {}
	if target == "panel" then
		local info = panel.info()
		ids = { info.id }
		rows[info.id] = {
			identifier = info.identifier,
			title = info.title,
			teamId = info.teamId,
			kind = info.state ~= nil and info.state.kind or nil,
		}
	elseif target == "cursor" then
		ids = { list.current().row.id }
	end
	panel.close()
	W.open, W.live, W.fixed, W.rows = true, live, ids, rows
	W.serial += 1
	-- (A start that is on its way is not forgotten: its answer comes to a form that may be gone.)
	W.asked, W.options, W.error = nil, nil, nil
	work.sync()
	return u:focus(prompt_name())
end

function work.close()
	W.open = false
end

-- Hands the issues over, with the words of the editor, to the program.
function work.start()
	if not work.startable() then
		return true
	end
	local name = prompt_name()
	local length = b.layout.editor_length(name) or 0
	local words = b.layout.editor_text(name, 1, length + 1) or ""
	W.starting, W.error = true, nil
	-- Status goes only where the form had the row, and then as it said: to move (true) or to leave
	-- (false). Without the row there is nothing to move and nothing to remember.
	local progress = nil
	if work.moving() ~= nil then
		progress = W.status == "progress"
	end
	local serial = W.serial
	kit.call("start", {
		ids = work.ids(),
		prompt = words,
		agent = W.agent,
		folder = W.folder,
		open = W.place,
		progress = progress,
	}, function(result, err)
		W.starting = false
		-- The form it was sent from may have been put away, and another opened: that one is not done.
		if W.serial ~= serial or not W.open then
			return true
		end
		if err then
			W.error = err.message or err.code
			return true
		end
		-- It is on its way: the form is done, and what was selected was for it. Where the issues were to
		-- move and Linear did not, the list's strip says so (the agent is started all the same).
		if result ~= nil and result.notMoved ~= nil then
			change.note(result.notMoved)
		end
		W.open = false
		list.clear_marks()
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

-- A line under a control: wrapped where it is words, or kept to one line where it is a name that is
-- cut short (`one_line`).
local function note(words, one_line)
	local text = b.text(words):text_size(SIZE.small):color("dim"):min_w(0):grow(1)
	text = one_line and text:truncate() or text:wrap()
	return b.div():row():gap(12):no_shrink():child(b.div():w(LABEL):no_shrink()):child(text)
end

-- The box that lets an issue go from the form (and from the selection).
local function leave_box(id, info)
	local name = "work-check:" .. id
	local ring = u.theme.ring
	-- The box has the keyboard in Tab's order, and enter or the space bar lets the issue go.
	return b.div()
		:row()
		:size(18, 18)
		:items_center()
		:justify_center()
		:rounded(6)
		:no_shrink()
		:border(1, u:focus_visible(name) and ring or u.theme.clear)
		:press(name)
		:focusable()
		:role("checkbox")
		:checked(true)
		:label("Leave out " .. info.identifier)
		:child(row.box(name, true))
end

-- An issue the form is for: its box (where they were selected, to let one go), identifier and title.
local function issue_line(id)
	local info = list.issue(id) or W.rows[id]
	if info == nil then
		return nil
	end
	local line = b.div():row():items_center():gap(10):h(26):no_shrink()
	if W.live then
		line:child(leave_box(id, info))
	end
	line:child(b.text(info.identifier):text_size(SIZE.small):color("dim"):no_shrink())
	line:child(b.text(info.title):grow(1):min_w(0):truncate())
	return line
end

-- The issues: a line each, and in a short pane one line of identifiers.
local function issues_box(ids, compact)
	local box = b.div():col():no_shrink()
	if compact then
		local line = b.div():row():wrap():gap_x(14):gap_y(4):no_shrink()
		for _, id in ids do
			local info = list.issue(id) or W.rows[id]
			if info ~= nil then
				local item = b.div():row():items_center():gap(7):no_shrink()
				if W.live then
					item:child(leave_box(id, info))
				end
				line:child(item:child(b.text(info.identifier):text_size(SIZE.small)))
			end
		end
		return box:child(line)
	end
	for _, id in ids do
		local line = issue_line(id)
		if line ~= nil then
			box:child(line)
		end
	end
	return box
end

-- The words for the agent: an editor the host keeps, that wraps, in a box of the field's look.
local function prompt_box(compact)
	local t = u.theme
	local name = u:field(prompt_name())
	local ringed = u:keyboard() == name
	local editor = b.editor(name, {
		wrap = true,
		gutter = false,
		padding = 8,
		line_height = 1.5,
		hint = "Tell the agent what to do (optional)",
		-- Tab is the page's: it goes on to the next field.
		keymap = { tab = false, ["shift+tab"] = false },
	}):grow(1):min_h(0):label("What the agent is to do")
	return b.div()
		:h(compact and 64 or 118)
		:no_shrink()
		:rounded(t.radius.control)
		:bg(t.field)
		:border(1, ringed and t.ring or t.line)
		:motion({ seconds = t.motion, color = true })
		:child(editor)
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
	local items = {
		{ key = "here", title = "This folder", tip = "Work in " .. (options.folder.name or "the folder linearpen began in") },
	}
	if options.worktree.available then
		items[#items + 1] = {
			key = "worktree",
			title = "New worktree",
			tip = "The agent makes a worktree first, by this project's own rules",
		}
	end
	return u:segments(FOLDER, { value = W.folder, items = items })
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
	elseif #work.ids() == 0 then
		why = "No issue is chosen"
	elseif #work.ids() > work.MOST then
		why = "At most " .. work.MOST .. " issues at a time: leave some out"
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

local function title_words(count)
	if count > 1 then
		return "Work on " .. count .. " issues"
	end
	return "Work on issue"
end

function work.view(width)
	local ids = work.ids()
	local compact = S.height < SIZE.short
	local options = chosen()
	local content = b.div():col():grow(1):min_w(0):min_h(0):pt(SIZE.head_top):px(SIZE.pad):gap(compact and 8 or 12)
	content:child(
		b.div():row():items_center():gap(8):no_shrink():child(
			b.text(title_words(#ids)):text_size(15):weight(700)
		):child(b.spacer()):child(kit.button("work-close", "Close", { suffix = "Esc" }))
	)
	content:child(issues_box(ids, compact))
	content:child(prompt_box(compact))
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
		content:child(field("Folder", folder_control(options)))
		-- (In a short pane the line under Folder is left out, for Start to stay on the page: the
		-- choice's tip says it.)
		if W.folder == "worktree" and not compact then
			content:child(note("The agent makes it, by this project's own rules", true))
		elseif not options.worktree.available and options.worktree.reason ~= nil then
			content:child(note("No new worktree: " .. options.worktree.reason))
		end
		content:child(field("Open in", place_control()))
		local moving = work.moving()
		if moving ~= nil then
			content:child(field("Status", status_control(moving)))
		end
	end
	local button, why = start_button()
	local foot = b.div():row():items_center():gap(12):no_shrink():child(button)
	if why ~= nil and #ids > 0 then
		foot:child(b.text(why):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1))
	end
	content:child(foot)
	if W.error ~= nil then
		content:child(b.text(W.error):text_size(SIZE.small):color("warning"):wrap())
	end
	local side = b.div():row():w(width):no_shrink():min_h(0)
	if width < S.width then
		side:child(b.div():w(1):bg(b.alpha("foreground", 0.23)):no_shrink())
	end
	return side:child(b.div():id("work-scroll"):scroll():col():grow(1):min_w(0):min_h(0):pb(SIZE.pad):child(content))
end

-- --- What the person does ------------------------------------------------------------------------

-- A press on one of the form's own buttons or boxes. The reply, or nil where it was not the form's.
function work.press(id)
	if id == "work-close" then
		work.close()
		return u:focus(nil)
	elseif id == "work-start" then
		return work.start()
	elseif string.sub(id, 1, 11) == "work-check:" then
		list.toggle_mark("i:" .. string.sub(id, 12))
		-- The box is gone with the issue: the keyboard goes back to the words for the agent.
		return u:focus(u:field(prompt_name()))
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

-- What Tab goes through, in order, as it stands now: the boxes that let an issue go, the words for the
-- agent, the choice made in each control (the one of its parts that is in the order of the keyboard), and the button that starts.
local function order()
	local names = {}
	if W.live then
		for _, id in work.ids() do
			local name = "work-check:" .. id
			if b.layout.bounds(name) ~= nil then
				names[#names + 1] = name
			end
		end
	end
	names[#names + 1] = u:field(prompt_name())
	local controls = { { AGENT, W.agent }, { FOLDER, W.folder }, { PLACE, W.place }, { STATUS, W.status } }
	for _, control in controls do
		local name = control[2] ~= nil and u:name(control[1], "item", control[2]) or nil
		if name ~= nil and b.layout.bounds(name) ~= nil then
			names[#names + 1] = name
		end
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

return work
