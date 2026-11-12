-- What the program tells and what the page asks of it (docs/calls.md): the key, the list, the view.

local kit = require("../lib/kit")
local list = require("./list")

local b, S, act = kit.b, kit.S, kit.act

local data = {}

-- Takes a list of the program's (the answer of `view` or a push of the topic).
local function apply(snapshot)
	S.issues = snapshot
	S.scope = snapshot.scope
	S.list_id = snapshot.listId
	S.catalog = snapshot.catalog
	S.refreshing = snapshot.refreshing == true
	list.load(snapshot)
	-- A new issue that is waited for is shown when it comes.
	if act.listed ~= nil then
		act.listed()
	end
	-- What the commands that change issues need is asked for as the lists come, not at the first key.
	if act.sync ~= nil then
		act.sync()
	end
end

-- Tells the program what the page looks at (`list`: `{ kind, id }`, `filters`, `filter`, `closed`). `after`, where there is one, is
-- called when the program has answered (or has not).
function act.view(params, after)
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
function act.refresh()
	if S.refreshing then
		return
	end
	S.refreshing = true
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
