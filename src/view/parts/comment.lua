-- The comment the person writes under the comments of an issue: a field of several lines, the words
-- kept for each issue while it is not on the page (they are never sent unless the person sends them),
-- and the call that posts them. The words are the person's: they go to the program as they were
-- written, once for each press of Comment, and what Linear does with them comes back as it is (the
-- comment, or why not, said under the field with the words kept).

local kit = require("../lib/kit")
local attach = require("./attach")
local commands = require("./commands")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local comment = {}

-- The most characters of a comment: the program says no to more.
local MOST = 10000

-- What the page holds: the words kept of each issue (`drafts`), the issue the editor drawn last is of
-- (`drawn`) and which editor it is (`serial`: another, empty, once a comment is posted), the issue whose
-- kept words are being put into an editor just made (`loading`: what the editor holds is not the
-- words yet, and is not kept), the version of each editor as it was last read, the issues whose
-- comment is on its way and why one was not posted.
local C = { drafts = {}, drawn = nil, serial = {}, loading = nil, seen = {}, sending = {}, problems = {} }
comment.state = C

local function editor_name(id)
	return "comment-" .. id .. "-" .. (C.serial[id] or 0)
end

-- The words the editor of the issue `id` holds now, or nil where it is not on the page.
local function words_of(id)
	local name = editor_name(id)
	local length = b.layout.editor_length(name)
	if length == nil then
		return nil
	end
	return b.layout.editor_text(name, 1, length + 1) or ""
end

local function trimmed(words)
	return string.match(words, "^%s*(.-)%s*$") or ""
end

-- Keeps the words of the editor drawn last, as they are: called before every event is done, while the
-- editor is on the page still (an event may take it away: another issue, a form, Close), so that what
-- was written is never lost to a change of the page. Nothing is read while the editor has not changed.
function comment.keep()
	local id = C.drawn
	if id == nil then
		return
	end
	local name = editor_name(id)
	local version = b.layout.editor_version(name)
	-- (A version below the one last read is another document's: the editor was made again, and is not
	-- read until it has said it is ready, which is when its kept words go in.)
	if version == nil or (C.seen[name] ~= nil and version <= C.seen[name]) then
		return
	end
	local words = words_of(id)
	if words == nil then
		return
	end
	if C.loading == id then
		-- The kept words are not in the editor yet: what it holds is not them.
		if words == "" then
			return
		end
		C.loading = nil
	end
	C.seen[name] = version
	C.drafts[id] = words
end

-- The editor was just made: the words kept of its issue are put in it, with the caret after them. The
-- reply, or nil.
function comment.ready(event)
	local id = C.drawn
	if id == nil or event.editor ~= editor_name(id) then
		return nil
	end
	C.seen[event.editor] = nil
	local kept = C.drafts[id]
	if kept == nil or kept == "" then
		C.loading = nil
		return nil
	end
	return { editor = { [event.editor] = { { load = kept }, { command = "move", movement = "document_end" } } } }
end

-- The editor of the comment changed (the person typed, undid, or the kept words were put in): a frame
-- is drawn, for what is in it says whether Comment can be pressed. The reply, or nil.
function comment.edited(event)
	if C.drawn ~= nil and event.editor == editor_name(C.drawn) then
		return true
	end
	return nil
end

-- Whether the keyboard is in the editor of the comment.
function comment.typing()
	return C.drawn ~= nil and u:keyboard() == editor_name(C.drawn)
end

-- The words that would be posted for the issue `id` (what the editor holds, else what was kept).
local function words_for(id)
	local live = words_of(id)
	if live ~= nil and C.loading ~= id then
		return live
	end
	return C.drafts[id] or ""
end

-- The key of the files chosen for the comment of the issue `id`.
local function pictures_of(id)
	return "comment:" .. id
end

-- Whether Comment can be pressed now for the issue `id`: there are words, or files (the comment may
-- be the files alone).
function comment.sendable(id)
	if id == nil or C.sending[id] then
		return false
	end
	local words = words_for(id)
	local length = utf8.len(words)
	return (trimmed(words) ~= "" or attach.count(pictures_of(id)) > 0) and length ~= nil and length <= MOST
end

-- Why Comment cannot be pressed, where it cannot.
local function why_not(id)
	if C.sending[id] then
		return nil
	end
	local words = words_for(id)
	if trimmed(words) == "" and attach.count(pictures_of(id)) == 0 then
		return "Write the comment or attach a file first"
	end
	return "A comment is up to " .. MOST .. " characters"
end

-- Posts the words of the issue `id`. They stay where they are until Linear says it posted them. The
-- reply.
function comment.send(id)
	if not comment.sendable(id) then
		return true
	end
	local words = words_for(id)
	C.sending[id], C.problems[id] = true, nil
	local files = attach.params(pictures_of(id))
	kit.call("comment", { issue = id, body = words, files = files }, function(result, err)
		C.sending[id] = nil
		if err then
			C.problems[id] = err.message or err.code
			-- The refusal is said under the field, which is at the end of the issue: it is brought into
			-- view, with Comment.
			act.panel_end()
			return true
		end
		-- Posted: the words are Linear's now, the field is another one with nothing in it, and the
		-- issue is asked for again, with the comment among its own.
		local had = comment.typing() and C.drawn == id
		attach.release(pictures_of(id), files)
		C.drafts[id] = nil
		C.serial[id] = (C.serial[id] or 0) + 1
		if C.loading == id then
			C.loading = nil
		end
		if not act.panel_reask(id, function() end) then
			act.panel_forget(id)
		end
		return had and u:focus(nil) or true
	end)
	return true
end

-- Gives the keyboard to the editor of the issue `id`. The reply.
function comment.focus(id)
	return u:focus(u:field(editor_name(id)))
end
act.comment_focus = comment.focus

-- A key while the editor has the keyboard: Esc takes it back (the words stay), Tab goes to Comment
-- where it can be pressed; every other key is the editor's. The reply, or nil.
function comment.key(event)
	if not comment.typing() then
		return nil
	end
	local mods = event.mods or {}
	if mods.command or mods.control or mods.alt then
		return nil
	end
	if event.key == "escape" and not mods.shift then
		return u:focus(nil)
	elseif event.key == "tab" then
		if not mods.shift and comment.sendable(C.drawn) then
			return u:focus("comment-send")
		end
		return true
	end
	return nil
end

-- --- Drawing -----------------------------------------------------------------------------------------

-- The field and Comment, for the issue `id`, under its comments.
function comment.view(id)
	local t = u.theme
	-- An editor that is not on the page yet (the issue was not open, or another was, or a form had its
	-- place) is made now, empty: the words kept are put in it when it says it is ready.
	local fresh = b.layout.editor_version(editor_name(id)) == nil
	if C.drawn ~= id or fresh then
		C.drawn = id
		C.seen[editor_name(id)] = nil
		local kept = C.drafts[id]
		C.loading = (kept ~= nil and kept ~= "") and id or nil
	end
	local name = u:field(editor_name(id))
	local sending = C.sending[id] == true
	local ringed = u:keyboard() == name
	local editor = b.editor(name, {
		wrap = true,
		gutter = false,
		padding = 8,
		line_height = 1.5,
		hint = "Leave a comment",
		read_only = sending,
		-- Tab is the page's: it goes on to Comment.
		keymap = { tab = false, ["shift+tab"] = false },
	}):grow(1):min_h(0):label("Comment")
	-- (The editor is as high as the box: a row that starts its items at the top gives it none.)
	local field = b.div()
		:row()
		:gap(6)
		:h(76)
		:no_shrink()
		:pr(8)
		:rounded(t.radius.control)
		:bg(t.field)
		:border(1, ringed and t.ring or t.line)
		:motion({ seconds = t.motion, color = true })
		:child(editor)
		:child(b.div():no_shrink():mt(8):child(u:kbd(commands.first("comment"))))
	-- (The box has a name: a drop on it is the comment's.)
	local box = b.div():id("comment-box"):col():gap(8):mt(14):no_shrink():child(field)
	local tray = attach.tray(pictures_of(id), sending)
	if tray ~= nil then
		box:child(tray)
	end
	local ready = comment.sendable(id)
	local button = kit.button("comment-send", "Comment", {
		primary = true,
		disabled = not ready,
		busy = sending,
		suffix = commands.label("comment_send"),
		tip = ready and "Post the comment to Linear" or why_not(id),
	})
	-- What Linear did not do is said at the start of the row of Comment, which stays where it is (a
	-- row that has more lines grows downward): a refusal moves nothing.
	local said = b.spacer()
	local problem = C.problems[id]
	if sending and attach.count(pictures_of(id)) > 0 then
		said = b.text(attach.uploading(pictures_of(id))):text_size(SIZE.small):color("dim"):grow(1):mt(7)
	elseif problem ~= nil then
		said = b.div():row():items_start():gap(8):grow(1):min_w(0):child(
			b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(11)
		):child(b.text(problem):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1):mt(7))
	end
	box:child(
		b.div():row():items_start():gap(12):no_shrink():child(said):child(attach.button("comment", sending)):child(button)
	)
	return box
end

return comment
