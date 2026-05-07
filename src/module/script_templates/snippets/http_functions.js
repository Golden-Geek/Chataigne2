/*
 * HTTP module functions (call them on local):
 *   local.request(method, path, body = "", contentType = "text/plain; charset=utf-8", headers = "", valuePath = "")
 *   local.requestJson(method, path, value = null, headers = "", valuePath = "")
 *   local.get(path, headers = "", valuePath = "")
 *   local.head(path, headers = "", valuePath = "")
 *   local.options(path, headers = "", valuePath = "")
 *   local.delete(path, headers = "", valuePath = "")
 *   local.post(path, body = "", contentType = "text/plain; charset=utf-8", headers = "", valuePath = "")
 *   local.postJson(path, value = null, headers = "", valuePath = "")
 *   local.put(path, body = "", contentType = "text/plain; charset=utf-8", headers = "", valuePath = "")
 *   local.putJson(path, value = null, headers = "", valuePath = "")
 *   local.patch(path, body = "", contentType = "text/plain; charset=utf-8", headers = "", valuePath = "")
 *   local.patchJson(path, value = null, headers = "", valuePath = "")
 *   local.uploadFile(method, path, filePath, fieldName = "file", contentType = "", headers = "", valuePath = "")
 *
 * Headers are one "Name: Value" line per header. Paths are resolved against
 * Parameters / Base address. JSON responses can auto-create Values entries.
 */

