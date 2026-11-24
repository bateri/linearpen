-- What the program tells and what the page asks of it (docs/calls.md): the key, the list, the view.

local kit = require("../lib/kit")
local list = require("./list")

local b, S, act = kit.b, kit.S, kit.act

local data = {}

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

-- Asks Linear for the lists again.
-- The ids of the issues whose notifications are being marked read, while Linear is asked.
local marking = {}

-- Whether the notifications of the issue `id` are being marked read now.
function act.marking(id)
	return marking[id] == true
end

-- Whether the issue `id` is a line of the inbox that is unread, and is not being marked read.
function act.unread(id)
	if id == nil or marking[id] or S.scope ~= "inbox" or S.issues == nil then
		return false
	end
	for _, group in S.issues.groups or {} do
		if group.key == "inbox:unread" then
			for _, row in group.rows or {} do
				if row.id == id then
					return true
				end
			end
			-- (A folded group draws no lines: what is not drawn is not marked from here.)
			return false
		end
	end
	return false
end

-- Marks the notifications of the issue `id` read in Linear's inbox (`mark_read`): once, when the person
-- asks; where Linear did not, it says so and the line stays as it was. The reply.
function act.mark_read(id)
	if id == nil or marking[id] then
		return true
	end
	marking[id] = true
	kit.call("mark_read", { id = id }, function(_, err)
		marking[id] = nil
		if err then
			kit.toast(err.message or "Linear did not mark it read", "warn")
		end
		return true
	end)
	return true
end

-- Keeps how wide what stands beside the list was left (`panel_width`); nothing is waited for.
function act.panel_width(width)
	kit.call("panel_width", { width = width })
end

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
