/**
 * @file optics_lab.h
 * @brief C Foreign Function Interface (FFI) ABI definition for the OpticsLab Optical Engine.
 *
 * Provides a pure C ABI boundary for embedding the computational optics engine
 * into C, C++, Python (via ctypes/cffi), MATLAB, and Julia environments.
 */

#ifndef OPTICS_LAB_H
#define OPTICS_LAB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------------- */
/* Error Codes                                                               */
/* ------------------------------------------------------------------------- */

/** Success status code. */
#define OPTICS_OK 0

/** Null pointer error status code. */
#define OPTICS_ERR_NULL_PTR -1

/** Invalid UTF-8 string encoding error status code. */
#define OPTICS_ERR_INVALID_UTF8 -2

/** JSON serialization or deserialization failure status code. */
#define OPTICS_ERR_JSON -3

/** General optical calculation or engine failure status code. */
#define OPTICS_ERR_ENGINE -4

/* ------------------------------------------------------------------------- */
/* Opaque Types                                                              */
/* ------------------------------------------------------------------------- */

/**
 * @brief Opaque handle to an OpticalEngine instance.
 */
typedef struct OpticsEngine OpticsEngine;

/* ------------------------------------------------------------------------- */
/* Lifecycle & Memory Management                                             */
/* ------------------------------------------------------------------------- */

/**
 * @brief Initializes a new OpticalEngine instance with bundled glass database and Cooke Triplet.
 * @return Opaque pointer to the engine, or NULL on error.
 */
OpticsEngine* optics_engine_create(void);

/**
 * @brief Initializes a new OpticalEngine instance loaded with a .zmx file content.
 * @param zmx_content Null-terminated UTF-8 string containing .zmx lens specification.
 * @return Opaque pointer to the engine, or NULL on error.
 */
OpticsEngine* optics_engine_create_with_zmx(const char* zmx_content);

/**
 * @brief Deallocates an OpticalEngine instance.
 * @param engine Opaque pointer to free. Safe to pass NULL.
 */
void optics_engine_free(OpticsEngine* engine);

/**
 * @brief Frees a string previously returned by any optics_* function.
 * @param s String pointer to deallocate. Safe to pass NULL.
 */
void optics_string_free(char* s);

/**
 * @brief Retrieves the last error message recorded on the current thread.
 * @return Null-terminated string describing the error (must be freed with optics_string_free), or NULL.
 */
char* optics_last_error_message(void);

/* ------------------------------------------------------------------------- */
/* System Mutation & Commands                                                */
/* ------------------------------------------------------------------------- */

/**
 * @brief Loads a .zmx lens prescription string into an existing engine.
 * @param engine Active engine handle.
 * @param zmx_content Null-terminated .zmx string.
 * @return OPTICS_OK on success, or negative error code.
 */
int optics_engine_load_zmx(OpticsEngine* engine, const char* zmx_content);

/**
 * @brief Returns the current engine status serialized as a JSON string.
 * @param engine Active engine handle.
 * @return JSON string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_get_status_json(OpticsEngine* engine);

/**
 * @brief Evaluates system diagnostics and returns an array of DiagnosticMessage JSON objects.
 * @param engine Active engine handle.
 * @return JSON string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_validate_json(OpticsEngine* engine);

/**
 * @brief Executes an editing command on the prescription using JSON serialized EngineCommand.
 * @param engine Active engine handle.
 * @param command_json JSON string specifying command and parameters.
 * @return JSON string containing {"success": bool, "message": string} or error details.
 */
char* optics_engine_execute_command_json(OpticsEngine* engine, const char* command_json);

/**
 * @brief Undoes the most recently executed command.
 * @param engine Active engine handle.
 * @return JSON status string (must be freed with optics_string_free).
 */
char* optics_engine_undo(OpticsEngine* engine);

/**
 * @brief Redoes the most recently undone command.
 * @param engine Active engine handle.
 * @return JSON status string (must be freed with optics_string_free).
 */
char* optics_engine_redo(OpticsEngine* engine);

/* ------------------------------------------------------------------------- */
/* Optical Analysis & Diagnostics                                            */
/* ------------------------------------------------------------------------- */

/**
 * @brief Performs an optical calculation specified by an AnalysisRequest JSON object.
 * @param engine Active engine handle.
 * @param request_json JSON string specifying analysis type (e.g. Paraxial, SpotDiagram, Mtf).
 * @return Serialized AnalysisResult JSON string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_run_analysis_json(OpticsEngine* engine, const char* request_json);

/* ------------------------------------------------------------------------- */
/* CAD & Manufacturing Export                                                */
/* ------------------------------------------------------------------------- */

/**
 * @brief Exports a 2D vector layout drawing in SVG format.
 * @param engine Active engine handle.
 * @return SVG XML string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_export_svg(OpticsEngine* engine);

/**
 * @brief Exports a 2D/3D layout in AutoCAD Release 12 DXF format.
 * @param engine Active engine handle.
 * @return ASCII DXF string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_export_dxf(OpticsEngine* engine);

/**
 * @brief Exports the optical prescription in .zmx format.
 * @param engine Active engine handle.
 * @return ZMX string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_export_zmx(OpticsEngine* engine);

/**
 * @brief Exports the optical system ISO 10110 manufacturing tables in Markdown format.
 * @param engine Active engine handle.
 * @return Markdown string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_export_iso10110_md(OpticsEngine* engine);

/**
 * @brief Exports the native OpticsLab project workspace schema in JSON format.
 * @param engine Active engine handle.
 * @return JSON string (must be freed with optics_string_free), or NULL on failure.
 */
char* optics_engine_export_project_json(OpticsEngine* engine);

/**
 * @brief Searches the active glass database by name substring.
 * @param engine Active engine handle.
 * @param query Glass name query string.
 * @return JSON array of matching GlassSummary objects (must be freed with optics_string_free).
 */
char* optics_engine_search_glasses_json(OpticsEngine* engine, const char* query);

#ifdef __cplusplus
}
#endif

#endif /* OPTICS_LAB_H */
