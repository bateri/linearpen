-- What the program tells and what the page asks of it (docs/calls.md): the key, the list, the view.

local kit = require("../lib/kit")
local list = require("./list")
local values = require("./values")

local b, S, act = kit.b, kit.S, kit.act

local data = {}

-- (Below: the cursor of a line marked read goes to the next unread when the lines say it was.)
local settle_moving

-- Takes a list of the program's (the answer of `view` or a push of the topic).
local function apply(snapshot)
	-- Whether the line over the page says Linear is asked for the lists (`page.lua`) as these come.
	local lit = S.refreshing and S.issues ~= nil and S.issues.listing == "ready"
	S.issues = snapshot
	S.scope = snapshot.scope
	S.list_id = snapshot.listId
	S.catalog = snapshot.catalog
	S.refreshing = snapshot.refreshing == true
	list.load(snapshot)
	settle_moving()
	-- What stands beside the list is of the list's kind (a project beside the projects).
	if act.lists_shown ~= nil then
		act.lists_shown()
	end
	-- A new issue that is waited for is shown when it comes.
	if act.listed ~= nil then
		act.listed()
	end
	-- The issue open is asked for again where the lists are new (the program let go of what it kept).
	if act.panel_listed ~= nil then
		act.panel_listed(lit)
	end
	-- What the commands that change issues need is asked for as the lists come, not at the first key.
	if act.sync ~= nil then
		act.sync()
	end
	if act.sync_filters ~= nil then
		act.sync_filters()
	end
end

-- Tells the program what the page looks at (`list`: `{ kind, id }`, `filters`, `filter`, `closed`). `after`, where there is one, is
-- called when the program has answered (or has not).
function act.view(params, after)
	if params.list ~= nil and act.list_going ~= nil then
		act.list_going(params.list)
	end
	-- Another list than a search is gone to: the words of the search were the search's, and go with it.
	if params.list ~= nil and params.list.kind ~= "search" and S.scope == "search" and params.filter == nil then
		params.filter = ""
		if act.field_clear ~= nil then
			act.field_clear()
		end
	end
	-- The calls run at the same time in the program: each is numbered, so that one told late does not
	-- undo a later one.
	S.view_seq = (S.view_seq or 0) + 1
	params.seq = S.view_seq
	-- The person's day, by their own clock: what a project's lateness is said by (nil where the host tells
	-- no time of day: the program takes the day of its own clock).
	params.today = values.day_ahead(0)
	kit.call("view", params, function(result, err)
		-- The answer of a call that a later one has gone past is not what the program holds now.
		if result and params.seq == S.view_seq then
			apply(result)
		end
		-- Filters the program did not take: the chips say what it holds, and the person is told.
		if err and params.filters ~= nil and act.filters_refused ~= nil then
			act.filters_refused(err)
		end
		if after ~= nil then
			after()
		end
		return true
	end)
end

-- The ids of the issues whose notifications are being marked read, while Linear is asked.
local marking = {}

-- Whether the notifications of the issue `id` are being marked read now.
function act.marking(id)
	return marking[id] == true
end

-- The lines of the unread in the inbox shown, or nil. (A folded group draws no lines: what is not drawn
-- is not marked from here.)
local function unread_rows()
	if S.scope ~= "inbox" or S.issues == nil then
		return nil
	end
	for _, group in S.issues.groups or {} do
		if group.key == "inbox:unread" then
			return group.rows or {}
		end
	end
	return nil
end

-- Where the issue `id` is among the unread lines, or nil.
local function unread_at(id)
	for index, row in unread_rows() or {} do
		if row.id == id then
			return index
		end
	end
	return nil
end

-- Whether the issue `id` is a line of the inbox that is unread, and is not being marked read.
function act.unread(id)
	return id ~= nil and not marking[id] and unread_at(id) ~= nil
end

-- The line the cursor goes to once the issue it is on is read: `{ id, to }`, the unread line after it
-- (or before it, where it was the last). Nil where nothing waits.
local moving = nil

-- The issue the cursor is on was marked read and has left the unread: the cursor goes to the next unread
-- line, whether the read ones are folded or not. Not where the person moved it meanwhile.
function settle_moving()
	if moving == nil then
		return
	end
	local item = list.current()
	if item == nil or item.key ~= "i:" .. moving.id then
		moving = nil
		return
	end
	if unread_at(moving.id) ~= nil then
		-- (Not yet: the lines that say it was read have not come.)
		return
	end
	local to = moving.to
	moving = nil
	if to ~= nil then
		list.go_to(to)
	end
end

-- Marks the notifications of the issue `id` read in Linear's inbox (`mark_read`): once, when the person
-- asks; where Linear did not, it says so and the line stays as it was. The reply.
function act.mark_read(id)
	-- (Only an unread line of the inbox: a line already read, or one being marked, sends nothing.)
	if not act.unread(id) then
		return true
	end
	local rows, at = unread_rows(), unread_at(id)
	local next_row = rows[at + 1] or rows[at - 1]
	local item = list.current()
	if item ~= nil and item.key == "i:" .. id then
		moving = { id = id, to = next_row and ("i:" .. next_row.id) or nil }
	end
	marking[id] = true
	kit.call("mark_read", { id = id }, function(_, err)
		marking[id] = nil
		if err then
			moving = nil
			kit.toast(err.message or "Linear did not mark it read", "warn")
		else
			settle_moving()
		end
		return true
	end)
	return true
end

-- Keeps how wide what stands beside the list was left (`panel_width`); nothing is waited for.
function act.panel_width(width)
	kit.call("panel_width", { width = width })
end

-- Asks Linear for the lists again.
function act.refresh()
	if S.refreshing then
		return
	end
	S.refreshing = true
	-- (What was told of a project beside the list of projects is asked for again too.)
	if act.lists_asked ~= nil then
		act.lists_asked()
	end
	kit.call("refresh", {}, function(_, err)
		if err then
			S.refreshing = false
		end
		return true
	end)
end

-- Tries the key typed on the Connect screen: the program asks Linear, keeps what it takes, and tells
-- the lists as they come.
function act.connect(text)
	if S.connecting then
		return
	end
	S.connecting, S.connect_error = true, nil
	kit.call("connect", { key = text }, function(result, err)
		S.connecting = false
		if err then
			S.connect_error = err.message or err.code
		else
			S.has_key = true
			S.saved_problem = result.saveProblem
		end
		return true
	end)
end

-- The keyboard goes to the key's field, which is what the Connect screen is for. The field is not
-- made yet, so the kit is asked to wait for it.
local function ask_for_key()
	kit.gather(kit.u:focus(kit.u:name("key", "field")))
end

-- The topic of the lists: told as the program has something new.
local function listen()
	b.subscribe("issues", {}, function(told)
		local before = S.refused
		apply(told)
		S.refused = told.error ~= nil and told.error.kind == "refused" and not told.loaded
		if S.refused and not before then
			ask_for_key()
		end
		return kit.take(true)
	end, function(err)
		S.topic_error = err.message or err.code
		return kit.take(true)
	end)
end

-- Boots: what the program knows of the key, and then the lists.
function data.start()
	kit.call("boot", {}, function(boot, err)
		if err then
			S.boot_error = err.message or err.code
			return true
		end
		S.boot = boot
		-- (A width the person left before; the page's width holds it.)
		if type(boot.panel) == "number" then
			act.keep_width(boot.panel)
		end
		S.has_key = boot.key ~= "none"
		S.key_problem = boot.problem
		if not S.has_key then
			ask_for_key()
		end
		listen()
		return true
	end)
end

return data
