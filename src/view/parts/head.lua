-- The head of the page: which issues (mine, or all of them, each with its count), how many the
-- filter lets through, the filter, and Refresh. Where the pane is narrow the filter has a row of
-- its own, and Refresh leaves for the key line.

local kit = require("../lib/kit")
local commands = require("./commands")
local list = require("./list")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local head = {}

-- The field's state: what it holds.
local F = { value = "" }
head.field = F

local FIELD = u:name("filter", "field")

-- Puts the keyboard in the field, for `/`.
function head.focus()
	return u:focus(FIELD)
end

-- Puts the keyboard back on the page, where the list has the arrows. The library is told too
-- (`u:focus(nil)`): a reply alone moves the host's keyboard and leaves the library sending Enter
-- and Esc to the field.
function head.leave()
	return u:focus(nil)
end

-- Puts `text` in the field and tells the program.
function head.set(text)
	F.value = text
	act.view({ filter = text })
end

-- Whether the keyboard is in the field.
function head.typing()
	return u.holder == FIELD
end

-- What the filter or the scope or the field did. Whether it was the head's, and its reply.
function head.action(action)
	local from, kind = action.from, action.kind
	if from == "filter" then
		if kind == "change" then
			head.set(action.text)
		elseif kind == "submit" then
			-- Enter in the field is Enter on the list: the issue the cursor is on opens, and the
			-- keyboard goes back to the page.
			local item = list.current()
			if item ~= nil and item.kind == "issue" then
				act.open_issue(item)
			end
			return head.leave()
		elseif kind == "escape" then
			-- The first escape clears the field, the next leaves it.
			if F.value ~= "" then
				head.set("")
			else
				return head.leave()
			end
		end
		return true
	elseif from == "filter-clear" and kind == "press" then
		head.set("")
		return head.focus()
	elseif from == "scope" and kind == "select" then
		act.view({ scope = action.key })
		return true
	end
	return nil
end

-- How many open issues a list holds, as the segment says it: "500+" where there are more than the
-- program takes.
local function count_words(snapshot, scope)
	local counts = snapshot ~= nil and snapshot.loaded and snapshot.counts or nil
	if counts == nil then
		return ""
	end
	local cut = scope == "mine" and counts.mineCut or scope == "all" and counts.allCut
	return " " .. tostring(scope == "mine" and counts.mine or counts.all) .. (cut and "+" or "")
end

local function scope_control()
	local snapshot = S.issues
	return u:segments("scope", {
		value = S.scope,
		items = {
			{ key = "mine", title = "My issues" .. count_words(snapshot, "mine"), tip = "Show my issues, key 1" },
			{ key = "all", title = "All issues" .. count_words(snapshot, "all"), tip = "Show all issues, key 2" },
		},
	})
end

-- "4 of 39" while the filter has words, what it lets through of what the list holds.
local function matched_words()
	local snapshot = S.issues
	if F.value == "" or snapshot == nil or not snapshot.loaded then
		return nil
	end
	local total = S.scope == "mine" and snapshot.counts.mine or snapshot.counts.all
	return string.format("%d of %d", snapshot.matched, total)
end

local function field_box()
	local trail = {}
	if F.value ~= "" then
		trail[1] = u:icon_button("filter-clear", { icon = "close", label = "Clear the filter", keys = "escape" })
	elseif not head.typing() then
		trail[1] = u:kbd(commands.first("filter"), { quiet = true })
	end
	return u:input("filter", {
		state = F,
		label = "Filter issues",
		hint = "Filter issues",
		lead = kit.icon("search", 14, "quiet"),
		trail = trail,
		escape = true,
	})
end

-- The button that asks Linear again: busy while it does.
local function refresh_button()
	local busy = S.refreshing
	return kit.button("refresh", "Refresh", {
		suffix = (not busy and not head.typing()) and commands.label("refresh") or nil,
		busy = busy,
		disabled = busy,
		tip = busy and "Linear is being asked" or "Ask Linear for the issues again",
	})
end

-- The button that hands issues to an agent, where it is the head's (the issue that is open has it
-- in its own panel). It stands by what it works on: by the count of the issues selected ("marks"),
-- or at the end of the head for the issue under the cursor, which it names. Where the cursor is on
-- a group's head there is no issue to name, and it waits there, not to be pressed.
local function work_button(target)
	local suffix = not head.typing() and commands.label("work") or nil
	if target == "marks" then
		return kit.button("work", "Work on them", {
			primary = true,
			small = true,
			suffix = suffix,
			tip = "Hand the issues selected to an agent",
		})
	end
	local item = list.current()
	local issue = target == "cursor" and item ~= nil and item.kind == "issue"
	return kit.button("work", issue and ("Work on " .. item.row.identifier) or "Work on issue", {
		primary = true,
		suffix = suffix,
		disabled = not issue,
		tip = issue and "Hand it to an agent" or "Put the cursor on an issue first",
	})
end

-- The button that opens the form of a new issue. Where the list has less room the key is in the line
-- of keys under the page.
local function new_button()
	return kit.button("new", "New issue", {
		suffix = not head.typing() and commands.label("new") or nil,
		tip = "Write an issue and add it to Linear",
	})
end

function head.view()
	local width = S.room
	local marked = list.marked()
	-- The filter has a row of its own where the head has no room for it: a narrow list, or one whose
	-- head holds what is selected too.
	local narrow = width < SIZE.narrow or (marked > 0 and width < 820)
	local target = act.work_target ~= nil and act.work_target() or nil
	local bar = b.div():row():items_center():gap(10):pt(SIZE.head_top):pb(SIZE.head_bottom):px(SIZE.pad):no_shrink()
	bar:child(scope_control())
	local words = matched_words()
	if marked > 0 then
		bar:child(b.text(marked .. " selected"):text_size(SIZE.small):color("accent"):no_shrink())
		if target == "marks" then
			bar:child(work_button(target))
		end
		-- With a panel open the Esc is the panel's: one key, one meaning at a time.
		if not S.panel_open then
			bar:child(kit.button("clear-selection", "Clear", { small = true, suffix = "Esc" }))
		end
	elseif words ~= nil then
		bar:child(b.text(words):text_size(SIZE.small):color("dim"):no_shrink())
	end
	bar:child(b.spacer())
	if not narrow then
		bar:child(b.div():grow(1):min_w(150):max_w(300):child(field_box()))
	end
	if width >= SIZE.wide then
		bar:child(refresh_button())
	end
	-- A form is open in the place of the buttons that open one. A list with no line has no issue to
	-- hand over: that button is not there, rather than dead.
	if not S.form_open and width >= SIZE.roomy then
		bar:child(new_button())
	end
	-- (The issue that is open has its button in its panel, and what is selected has it by its count.)
	if not S.form_open and list.count() > 0 and marked == 0 and not S.panel_open then
		bar:child(work_button(target))
	end
	if not narrow then
		return bar
	end
	local box = b.div():col():no_shrink():child(bar)
	box:child(b.div():px(SIZE.pad):pb(8):child(field_box()))
	return box
end

return head
