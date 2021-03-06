/*
 * Copyright (c) 2010, Stefan Lankes, RWTH Aachen University
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *    * Redistributions of source code must retain the above copyright
 *      notice, this list of conditions and the following disclaimer.
 *    * Redistributions in binary form must reproduce the above copyright
 *      notice, this list of conditions and the following disclaimer in the
 *      documentation and/or other materials provided with the distribution.
 *    * Neither the name of the University nor the names of its contributors
 *      may be used to endorse or promote products derived from this
 *      software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
 * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE REGENTS OR CONTRIBUTORS BE LIABLE FOR ANY
 * DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
 * LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
 * ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
 * SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#ifndef __MAILBOX_H__
#define __MAILBOX_H__

#include <hermit/string.h>
#include <hermit/mailbox_types.h>
#include <hermit/syscall.h>
#include <hermit/errno.h>

#ifdef __cplusplus
extern "C" {
#endif

#define MAILBOX(name, type) 	\
	inline static int mailbox_##name##_init(mailbox_##name##_t* m) { \
		if (BUILTIN_EXPECT(!m, 0)) \
			return -EINVAL; \
	\
		memset(m->buffer, 0x00, sizeof(type)*MAILBOX_SIZE); \
		m->wpos = m->rpos = 0; \
		sys_sem_init(&m->mails, 0); \
		sys_sem_init(&m->boxes, MAILBOX_SIZE); \
		sys_spinlock_init(&m->rlock); \
		sys_spinlock_init(&m->wlock); \
	\
		return 0; \
	}\
	\
	inline static int mailbox_##name##_destroy(mailbox_##name##_t* m) { \
		if (BUILTIN_EXPECT(!m, 0)) \
			return -EINVAL; \
	\
		sys_sem_destroy(m->mails); \
		sys_sem_destroy(m->boxes); \
		sys_spinlock_destroy(m->rlock); \
		sys_spinlock_destroy(m->wlock); \
	\
		return 0; \
	} \
	\
	inline static int mailbox_##name##_post(mailbox_##name##_t* m, type mail) { \
		if (BUILTIN_EXPECT(!m, 0)) \
			return -EINVAL; \
	\
		sys_sem_timedwait(m->boxes, 0); \
		sys_spinlock_lock(m->wlock); \
		m->buffer[m->wpos] = mail; \
		m->wpos = (m->wpos+1) % MAILBOX_SIZE; \
		sys_spinlock_unlock(m->wlock); \
		sys_sem_post(m->mails); \
	\
		return 0; \
	} \
	\
	inline static int mailbox_##name##_trypost(mailbox_##name##_t* m, type mail) { \
		if (BUILTIN_EXPECT(!m, 0)) \
			return -EINVAL; \
	\
		if (sys_sem_trywait(m->boxes)) \
			return -EBUSY; \
		sys_spinlock_lock(m->wlock); \
		m->buffer[m->wpos] = mail; \
		m->wpos = (m->wpos+1) % MAILBOX_SIZE; \
		sys_spinlock_unlock(m->wlock); \
		sys_sem_post(m->mails); \
	\
		return 0; \
	} \
	\
	inline static int mailbox_##name##_fetch(mailbox_##name##_t* m, type* mail, uint32_t ms) { \
		int err; \
	\
		if (BUILTIN_EXPECT(!m || !mail, 0)) \
			return -EINVAL; \
	\
		err = sys_sem_timedwait(m->mails, ms); \
		if (err) return SYS_ARCH_TIMEOUT; \
		sys_spinlock_lock(m->rlock); \
		*mail = m->buffer[m->rpos]; \
		m->rpos = (m->rpos+1) % MAILBOX_SIZE; \
		sys_spinlock_unlock(m->rlock); \
		sys_sem_post(m->boxes); \
	\
		return 0; \
	} \
	\
	inline static int mailbox_##name##_tryfetch(mailbox_##name##_t* m, type* mail) { \
		if (BUILTIN_EXPECT(!m || !mail, 0)) \
			return -EINVAL; \
	\
		if (sys_sem_trywait(m->mails) != 0) \
			return -EINVAL; \
		sys_spinlock_lock(m->rlock); \
		*mail = m->buffer[m->rpos]; \
		m->rpos = (m->rpos+1) % MAILBOX_SIZE; \
		sys_spinlock_unlock(m->rlock); \
		sys_sem_post(m->boxes); \
	\
		return 0; \
	}\

MAILBOX(ptr, void*)

#ifdef __cplusplus
}
#endif

#endif
