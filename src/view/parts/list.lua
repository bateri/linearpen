-- The list: the groups of the issues as lines, the line the cursor is on, and what moves it and what
-- a press on a line does (a group's line folds it; an issue's is chosen). The program holds the
-- model (what the filter lets through, the groups, the order); the page keeps the cursor and draws
-- the lines that are in view.

local kit = require("../lib/kit")
local virtual = require("../lib/virtual")
local row = require("./row")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local list = {}

local ID = "list"
local HEAD = SIZE.row + SIZE.group_gap

-- The lines as the program last told them: `items` one for each (`{ kind, key, group, row }`),
-- `tops` where each starts (and where the last ends), `at` the place of a key, and the groups that
-- are further along than the first of their kind.
local L = { items = {}, tops = { 0 }, at = {}, late = {}, cursor = nil, cursor_group = nil, marks = {} }
list.state = L

local function index_of(key)
	return key ~= nil and L.at[key] or nil
end

-- What the program told: the lines made again from it, the cursor kept where it still stands (or on
-- the head of its group, where its line is gone with a fold, or on the first issue).
function list.load(snapshot)
	local items, tops, at, late = {}, { 0 }, {}, {}
	local seen_started = false
	local first_issue = nil
	for _, group in snapshot.groups do
		if group.kind == "started" then
			late[group.key] = seen_started
			seen_started = true
		end
		local key = "g:" .. group.key
		items[#items + 1] = { kind = "group", key = key, group = group }
		at[key] = #items
		tops[#items + 1] = tops[#items] + HEAD
		for _, issue in group.rows or {} do
			local issue_key = "i:" .. issue.id
			items[#items + 1] = { kind = "issue", key = issue_key, row = issue, group = group }
			at[issue_key] = #items
			tops[#items + 1] = tops[#items] + SIZE.row
			first_issue = first_issue or issue_key
		end
	end
	L.items, L.tops, L.at, L.late = items, tops, at, late
	if L.at[L.cursor] == nil then
		-- Where the cursor's line is gone: on the head of its group where the group is folded, else
		-- (a filter took the issue away) on the first issue.
		local head = L.cursor_group ~= nil and items[at["g:" .. L.cursor_group] or 0] or nil
		L.cursor = head ~= nil and head.group.closed and head.key or first_issue or (head and head.key)
	end
	local found = L.items[L.at[L.cursor] or 0]
	L.cursor_group = found and found.group.key or nil
	-- What is selected is only what can still be seen: a line a filter or a fold took away is not.
	for key in L.marks do
		if L.at[key] == nil then
			L.marks[key] = nil
		end
	end
end

-- How many issues are selected.
function list.marked()
	local count = 0
	for _ in L.marks do
		count += 1
	end
	return count
end

-- The ids of the issues selected, in the order of the list.
function list.marked_ids()
	local ids = {}
	for _, item in L.items do
		if item.kind == "issue" and L.marks[item.key] then
			ids[#ids + 1] = item.row.id
		end
	end
	return ids
end

-- Selects or lets go of the issue of `key`.
function list.toggle_mark(key)
	local item = L.items[L.at[key] or 0]
	if item == nil or item.kind ~= "issue" then
		return false
	end
	L.marks[key] = (not L.marks[key]) or nil
	return true
end

-- Selects the issue of the cursor's line; returns whether there is one.
function list.mark_current()
	local item = list.current()
	if item ~= nil and item.kind == "issue" then
		L.marks[item.key] = true
		return true
	end
	return false
end

function list.select_all()
	for _, item in L.items do
		if item.kind == "issue" then
			L.marks[item.key] = true
		end
	end
end

function list.clear_marks()
	table.clear(L.marks)
end

-- The line of the issue with Linear's `id`, or nil.
function list.issue(id)
	local item = L.items[L.at["i:" .. id] or 0]
	return item ~= nil and item.row or nil
end

function list.count()
	return #L.items
end

-- The item the cursor is on, or nil.
function list.current()
	return L.items[L.at[L.cursor] or 0]
end

-- Puts the cursor on `key` and scrolls it into view.
local function put(key)
	local index = index_of(key)
	if index == nil then
		return false
	end
	L.cursor = key
	L.cursor_group = L.items[index].group.key
	virtual.reveal(ID, index, #L.items, SIZE.row, 0, L.tops)
	return true
end

-- Puts the cursor on the issue with Linear's `id` and scrolls it into view. Its line, or nil where it
-- is not in the list.
function list.reveal_issue(id)
	local key = "i:" .. id
	if not put(key) then
		return nil
	end
	return L.items[L.at[key]]
end

-- Moves the cursor as `name` says ("down", "up", "first", "last", "page_down", "page_up"). Whether it
-- moved.
function list.move(name)
	local count = #L.items
	if count == 0 then
		return false
	end
	local at = index_of(L.cursor)
	local page = math.max(math.floor((virtual.views[ID] or 480) / SIZE.row) - 1, 1)
	local to
	if name == "down" then
		to = at and at + 1 or 1
	elseif name == "up" then
		to = at and at - 1 or count
	elseif name == "first" then
		to = 1
	elseif name == "last" then
		to = count
	elseif name == "page_down" then
		to = at and at + page or 1
	else
		to = at and at - page or count
	end
	to = math.min(math.max(to, 1), count)
	local moved = L.items[to].key ~= L.cursor
	put(L.items[to].key)
	return moved
end

-- Folds (`closed` true) or opens (false) the group of the cursor's line, or turns it over (nil);
-- the program is asked and tells the new list.
function list.fold(closed)
	local item = list.current()
	if item == nil then
		return false
	end
	local group = item.group
	if closed == nil then
		closed = not group.closed
	end
	if closed == (group.closed == true) then
		return false
	end
	-- The cursor goes to the head of the group it folds, where it will be when the lines are gone.
	if closed then
		L.cursor, L.cursor_group = "g:" .. group.key, group.key
	end
	S.closed[group.key] = closed
	act.view({ closed = S.closed })
	return true
end

-- A press on the lines: the one it landed on takes the cursor, and a group's head turns over too.
-- Whether the press was on a line.
function list.press(event)
	if (event.button or "left") ~= "left" then
		return false
	end
	local hit = b.layout.hit({ x = event.x, y = event.y })
	if hit ~= nil and string.sub(hit, 1, 6) == "check:" then
		-- (While the form is open what it is for is let go of by its own boxes, which the keyboard
		-- reaches too: the list's boxes are not a way to add to it, which only the pointer could do.)
		if S.work_open then
			return true
		end
		local box = string.sub(hit, 7)
		put(box)
		list.toggle_mark(box)
		return true
	end
	if hit == nil or string.sub(hit, 1, 4) ~= "row:" then
		return false
	end
	local key = string.sub(hit, 5)
	if index_of(key) == nil then
		return false
	end
	put(key)
	local item = L.items[index_of(key)]
	if item.kind == "group" then
		-- A double press is one press on a head that turned over once.
		if (event.count or 1) < 2 then
			list.fold(nil)
		end
	elseif event.count == 2 then
		act.open_issue(item)
	end
	return true
end

-- Enter on the cursor's line: a group's head turns over, and an issue opens beside the list.
function list.open()
	local item = list.current()
	if item == nil then
		return false
	end
	if item.kind == "group" then
		return list.fold(nil)
	end
	act.open_issue(item)
	return true
end

-- A key for the cursor. `in_field` is the keyboard being in a field: then only the arrows are the
-- list's, and the words stay in the field. Whether the key was taken.
local NAMES = { up = "up", down = "down", home = "first", ["end"] = "last", page_up = "page_up", page_down = "page_down" }
local LETTERS = { j = "down", k = "up" }

function list.key(event, in_field)
	local mods = event.mods or {}
	if mods.command or mods.control or mods.alt then
		return false
	end
	local name = NAMES[event.key]
	if name == nil and not in_field and not mods.shift then
		name = LETTERS[event.key]
	end
	if name ~= nil then
		if in_field and name ~= "up" and name ~= "down" then
			return false
		end
		if mods.shift and (name == "up" or name == "down") then
			-- Shift and an arrow selects the line it leaves and the one it comes to.
			list.mark_current()
			list.move(name)
			list.mark_current()
			return true
		end
		list.move(name)
		return true
	end
	if event.key == "enter" and not in_field then
		list.open()
		return true
	end
	return false
end

-- The lines in view, and the room under them.
function list.view(room, panel_open)
	local any = list.marked() > 0
	local o = {
		scope = S.scope,
		labels = not panel_open and room >= SIZE.labels,
		who = not panel_open and room >= SIZE.assignee,
	}
	return virtual.virtual(ID, #L.items, SIZE.row, function(index)
		local item = L.items[index]
		local name = row.name(item.key)
		local selected = item.key == L.cursor
		local look = {
			selected = selected,
			hovered = u:hovered(name),
			marked = L.marks[item.key] == true,
			working = item.kind == "issue" and S.issues.working ~= nil and S.issues.working[item.row.id] or nil,
			check = any or selected or u:hovered(name) or u:hovered(row.check_name(item.key)),
		}
		if item.kind == "group" then
			return row.group(item, look, L.late[item.group.key] == true)
		end
		return row.issue(item, look, o)
	end, { tops = L.tops, px = 0, trail = 6 })
end

-- The room the list has to scroll in, as laid out: what `reveal` needs.
function list.settle()
	local extent = b.layout.scroll_extent(ID)
	if extent ~= nil and virtual.views[ID] ~= extent.view then
		virtual.views[ID] = extent.view
		return true
	end
	return false
end

return list
