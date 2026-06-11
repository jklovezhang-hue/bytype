import { invoke } from "@tauri-apps/api/core";

export interface MeetingSummary { base: string; has_md: boolean; has_mp3: boolean; }
export interface MeetingDetail { base: string; md: string; has_json: boolean; has_mp3: boolean; }

export const listMeetings = () => invoke<MeetingSummary[]>("list_meetings");
export const getMeeting = (base: string) => invoke<MeetingDetail>("get_meeting", { base });
export const regenerateMinutes = (base: string) => invoke<string>("regenerate_minutes", { base });
export const deleteMeeting = (base: string) => invoke<void>("delete_meeting", { base });
export const openMeetingFolder = (base: string) => invoke<void>("open_meeting_folder", { base });
