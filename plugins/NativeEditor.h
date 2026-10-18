#pragma once
#ifdef _WIN32
#include <windows.h>
#include <string>

namespace keklocator {
inline constexpr WORD kEditorResource = 101;

inline HMODULE EditorModule() {
  HMODULE module = nullptr;
  GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                        GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                    reinterpret_cast<LPCWSTR>(&kEditorResource), &module);
  return module;
}

inline std::string EditorHtml(HMODULE module) {
  if (!module) return {};
  const auto resource = FindResourceW(module, MAKEINTRESOURCEW(kEditorResource), MAKEINTRESOURCEW(10));
  if (!resource) return {};
  const auto loaded = LoadResource(module, resource);
  if (!loaded) return {};
  const auto data = static_cast<const char*>(LockResource(loaded));
  const auto size = SizeofResource(module, resource);
  return data && size ? std::string(data, size) : std::string{};
}
} // namespace keklocator
#endif
