struct Handle {
  uint m_packed;

  bool valid() {
    return id() != 0xFFFFu;
  }

  int id() {
    return int(m_packed & 0xFFFF);
  }
};
