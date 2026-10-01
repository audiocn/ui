"use client";

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
} from "react";

import { clamp } from "@/lib/audio/decibels";

export interface MixerChannelState {
  id: string;
  /** Channel gain in dB. */
  gainDb: number;
  muted: boolean;
  solo: boolean;
  /** −1 (left) to 1 (right). */
  pan: number;
  /** Send the channel to the speakers. */
  monitor: boolean;
}

export interface MixerMasterState {
  gainDb: number;
  muted: boolean;
}

export interface MixerState {
  channels: MixerChannelState[];
  master: MixerMasterState;
}

export type MixerChannelInit = Partial<MixerChannelState> & { id: string };

export interface UseMixerOptions {
  /** Initial channels. Missing fields get defaults. */
  channels?: MixerChannelInit[];
  /** Initial master settings. */
  master?: Partial<MixerMasterState>;
  /** Controlled state. */
  state?: MixerState;
  onStateChange?: (state: MixerState) => void;
  /** Save state to `localStorage` under this key. */
  persistKey?: string;
}

type MixerAction =
  | { type: "replace"; state: MixerState }
  | {
      type: "channel";
      id: string;
      patch: Partial<Omit<MixerChannelState, "id">>;
    }
  | { type: "solo"; id: string; solo: boolean; exclusive: boolean }
  | { type: "master"; patch: Partial<MixerMasterState> }
  | { type: "add"; channel: MixerChannelInit }
  | { type: "remove"; id: string };

const DEFAULT_MASTER: MixerMasterState = { gainDb: 0, muted: false };

const createChannel = (init: MixerChannelInit): MixerChannelState => ({
  gainDb: 0,
  monitor: false,
  muted: false,
  pan: 0,
  solo: false,
  ...init,
});

const createState = (
  channels: MixerChannelInit[] = [],
  master: Partial<MixerMasterState> = {}
): MixerState => ({
  channels: channels.map(createChannel),
  master: { ...DEFAULT_MASTER, ...master },
});

/** The pure mixer reducer, exported for use outside React. */
export const mixerReducer = (
  state: MixerState,
  action: MixerAction
): MixerState => {
  switch (action.type) {
    case "replace": {
      return action.state;
    }
    case "channel": {
      return {
        ...state,
        channels: state.channels.map((channel) => {
          if (channel.id !== action.id) {
            return channel;
          }
          const next = { ...channel, ...action.patch };
          return { ...next, pan: clamp(next.pan, -1, 1) };
        }),
      };
    }
    case "solo": {
      return {
        ...state,
        channels: state.channels.map((channel) => {
          if (channel.id === action.id) {
            return { ...channel, solo: action.solo };
          }
          if (action.exclusive && action.solo) {
            return { ...channel, solo: false };
          }
          return channel;
        }),
      };
    }
    case "master": {
      return { ...state, master: { ...state.master, ...action.patch } };
    }
    case "add": {
      if (state.channels.some((channel) => channel.id === action.channel.id)) {
        return state;
      }
      return {
        ...state,
        channels: [...state.channels, createChannel(action.channel)],
      };
    }
    case "remove": {
      return {
        ...state,
        channels: state.channels.filter((channel) => channel.id !== action.id),
      };
    }
    default: {
      return state;
    }
  }
};

/** Whether a channel is heard: not muted, and soloed if anything is soloed. */
export const isChannelAudible = (state: MixerState, id: string): boolean => {
  const channel = state.channels.find((item) => item.id === id);
  if (!channel || channel.muted) {
    return false;
  }
  const anySolo = state.channels.some((item) => item.solo);
  return !anySolo || channel.solo;
};

const readPersisted = (key: string): MixerState | null => {
  try {
    const raw = window.localStorage.getItem(key);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as MixerState;
    return Array.isArray(parsed.channels) && parsed.master ? parsed : null;
  } catch {
    return null;
  }
};

const writePersisted = (key: string, state: MixerState) => {
  try {
    window.localStorage.setItem(key, JSON.stringify(state));
  } catch {
    // Storage can be full or blocked; the mixer keeps working without it.
  }
};

export interface Mixer {
  state: MixerState;
  channels: MixerChannelState[];
  master: MixerMasterState;
  channel: (id: string) => MixerChannelState | undefined;
  setGain: (id: string, gainDb: number) => void;
  setMuted: (id: string, muted: boolean) => void;
  setSolo: (
    id: string,
    solo: boolean,
    options?: { exclusive?: boolean }
  ) => void;
  setPan: (id: string, pan: number) => void;
  setMonitor: (id: string, monitor: boolean) => void;
  setMasterGain: (gainDb: number) => void;
  setMasterMuted: (muted: boolean) => void;
  /** False when muted, or when another channel is soloed. */
  isAudible: (id: string) => boolean;
  /** True when a channel is silenced only because another is soloed. */
  isDimmed: (id: string) => boolean;
  addChannel: (channel: MixerChannelInit) => void;
  removeChannel: (id: string) => void;
  reset: () => void;
}

/** State for a mixer: gain, mute, solo, pan and monitor per channel, plus a master. */
const initStore = (options: UseMixerOptions) => {
  const initial = createState(options.channels, options.master);
  return { current: initial, initial };
};

const storeReducer = (
  store: { current: MixerState; initial: MixerState },
  next: MixerState
) => (next === store.current ? store : { ...store, current: next });

export const useMixer = (options: UseMixerOptions = {}): Mixer => {
  const { state: controlledState, onStateChange, persistKey } = options;
  const [store, replace] = useReducer(storeReducer, options, initStore);
  const { initial } = store;
  const state = controlledState ?? store.current;
  const controlled = controlledState !== undefined;
  const stateRef = useRef(state);
  const onChangeRef = useRef(onStateChange);

  useLayoutEffect(() => {
    stateRef.current = state;
    onChangeRef.current = onStateChange;
  });

  const commit = useCallback(
    (next: MixerState) => {
      if (next === stateRef.current) {
        return;
      }
      stateRef.current = next;
      if (!controlled) {
        replace(next);
        // Saved on change, not from an effect: an effect would also write the
        // defaults on mount, over what the restore below is about to load.
        if (persistKey) {
          writePersisted(persistKey, next);
        }
      }
      onChangeRef.current?.(next);
    },
    [controlled, persistKey]
  );

  const dispatch = useCallback(
    (action: MixerAction) => {
      commit(mixerReducer(stateRef.current, action));
    },
    [commit]
  );

  useEffect(() => {
    if (!persistKey || controlled) {
      return;
    }
    const persisted = readPersisted(persistKey);
    if (persisted) {
      replace(persisted);
    }
  }, [controlled, persistKey]);

  return useMemo<Mixer>(() => {
    const anySolo = state.channels.some((channel) => channel.solo);
    return {
      addChannel: (channel) => dispatch({ channel, type: "add" }),
      channel: (id) => state.channels.find((channel) => channel.id === id),
      channels: state.channels,
      isAudible: (id) => isChannelAudible(state, id),
      isDimmed: (id) => {
        const channel = state.channels.find((item) => item.id === id);
        return Boolean(channel && !channel.muted && anySolo && !channel.solo);
      },
      master: state.master,
      removeChannel: (id) => dispatch({ id, type: "remove" }),
      reset: () => commit(initial),
      setGain: (id, gainDb) =>
        dispatch({ id, patch: { gainDb }, type: "channel" }),
      setMasterGain: (gainDb) =>
        dispatch({ patch: { gainDb }, type: "master" }),
      setMasterMuted: (muted) => dispatch({ patch: { muted }, type: "master" }),
      setMonitor: (id, monitor) =>
        dispatch({ id, patch: { monitor }, type: "channel" }),
      setMuted: (id, muted) =>
        dispatch({ id, patch: { muted }, type: "channel" }),
      setPan: (id, pan) => dispatch({ id, patch: { pan }, type: "channel" }),
      setSolo: (id, solo, soloOptions = {}) =>
        dispatch({
          exclusive: soloOptions.exclusive ?? false,
          id,
          solo,
          type: "solo",
        }),
      state,
    };
  }, [commit, dispatch, initial, state]);
};
